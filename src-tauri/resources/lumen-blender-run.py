# Lanceur Lumen : exécute un script bpy puis exporte le modèle.
# Appelé par :
#   blender --background --factory-startup --python lumen-blender-run.py -- \
#       --script X.py --out Y.glb [--renders DOSSIER] [--report R.json] [--srgb-colors] [--views front,back,...]
# --out en .fbx exporte en FBX (UEFN / Unreal), sinon GLB.
import json
import math
import struct
import sys
from pathlib import Path

argv = sys.argv
args = argv[argv.index("--") + 1 :] if "--" in argv else argv[1:]
opts = {"script": None, "out": None, "renders": None, "report": None, "views": "front,back,left,right,threequarter,top"}
flags = set()
i = 0
while i < len(args):
    key = args[i].lstrip("-")
    if key in opts and i + 1 < len(args):
        opts[key] = args[i + 1]
        i += 2
        continue
    flags.add(key)
    i += 1

if not opts["script"] or not opts["out"]:
    raise SystemExit("Lumen blender-run : --script et --out requis")

# lumen3d.py (aides low poly) est à côté de ce lanceur.
sys.path.insert(0, str(Path(__file__).resolve().parent))

import bpy
from mathutils import Vector

for obj in list(bpy.data.objects):
    bpy.data.objects.remove(obj, do_unlink=True)
for mesh in list(bpy.data.meshes):
    bpy.data.meshes.remove(mesh)
for mat in list(bpy.data.materials):
    bpy.data.materials.remove(mat)

script_path = Path(opts["script"]).resolve()
source = script_path.read_text(encoding="utf-8")
ns = {"__name__": "__main__", "__file__": str(script_path)}
exec(compile(source, str(script_path), "exec"), ns)

for obj in list(bpy.data.objects):
    if obj.type in {"CAMERA", "LIGHT"}:
        bpy.data.objects.remove(obj, do_unlink=True)
bpy.context.view_layer.update()


# ---------- export ----------

def linear_to_srgb(c):
    c = max(0.0, min(1.0, c))
    return c * 12.92 if c <= 0.0031308 else 1.055 * (c ** (1 / 2.4)) - 0.055


def srgb_vertex_colors(path):
    """glTF stocke COLOR_0 en linéaire ; Roblox l'affiche tel quel (trop sombre). On repasse en sRGB."""
    data = bytearray(Path(path).read_bytes())
    off, chunks = 12, []
    while off < len(data):
        length, kind = struct.unpack_from("<II", data, off)
        chunks.append([kind, bytearray(data[off + 8 : off + 8 + length])])
        off += 8 + length
    doc = json.loads(chunks[0][1].decode("utf-8").rstrip(" \x00"))
    binary = chunks[1][1] if len(chunks) > 1 else bytearray()
    done = set()
    for mesh in doc.get("meshes", []):
        for prim in mesh.get("primitives", []):
            index = prim.get("attributes", {}).get("COLOR_0")
            if index is None or index in done:
                continue
            done.add(index)
            acc = doc["accessors"][index]
            view = doc["bufferViews"][acc["bufferView"]]
            comps = {"VEC3": 3, "VEC4": 4}[acc["type"]]
            fmt, size, scale = {5126: ("<f", 4, 1.0), 5121: ("<B", 1, 255.0), 5123: ("<H", 2, 65535.0)}[acc["componentType"]]
            stride = view.get("byteStride", size * comps)
            base = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
            for n in range(acc["count"]):
                for c in range(3):
                    at = base + n * stride + c * size
                    value = struct.unpack_from(fmt, binary, at)[0] / scale
                    value = linear_to_srgb(value)
                    struct.pack_into(fmt, binary, at, value if scale == 1.0 else int(round(value * scale)))
    out = bytearray(data[:12])
    for kind, body in chunks:
        if kind == 0x4E4F534A:
            body = body  # JSON inchangé
        out += struct.pack("<II", len(body), kind) + body
    struct.pack_into("<I", out, 8, len(out))
    Path(path).write_bytes(out)
    return len(done)


out_path = Path(opts["out"]).resolve()
out_path.parent.mkdir(parents=True, exist_ok=True)
if out_path.suffix.lower() == ".fbx":
    bpy.ops.export_scene.fbx(
        filepath=str(out_path),
        use_selection=False,
        apply_scale_options="FBX_SCALE_ALL",
        mesh_smooth_type="FACE",
        colors_type="SRGB",
        bake_anim=True,
        add_leaf_bones=False,
    )
    fixed = 0
else:
    bpy.ops.export_scene.gltf(
        filepath=str(out_path),
        export_format="GLB",
        export_apply=True,
        export_yup=True,
        export_extras=True,
    )
    fixed = srgb_vertex_colors(out_path) if "srgb-colors" in flags else 0


# ---------- rapport ----------

def world_bbox(objs):
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in objs:
        for corner in o.bound_box:
            p = o.matrix_world @ Vector(corner)
            lo = Vector(map(min, lo, p))
            hi = Vector(map(max, hi, p))
    return lo, hi


def r3(v):
    return [round(x, 3) for x in v]


meshes = [o for o in bpy.data.objects if o.type == "MESH"]
depsgraph = bpy.context.evaluated_depsgraph_get()
rows, warnings, total = [], [], 0
for o in meshes:
    ev = o.evaluated_get(depsgraph)
    data = ev.to_mesh()
    tris = sum(len(p.vertices) - 2 for p in data.polygons)
    has_colors = len(data.color_attributes) > 0
    ev.to_mesh_clear()
    total += tris
    lo, hi = world_bbox([o])
    rows.append({
        "name": o.name,
        "parent": o.parent.name if o.parent else None,
        "tris": tris,
        "size": r3(hi - lo),
        "pivot": r3(o.matrix_world.translation),
        "colors": has_colors,
    })
    if tris > 10000:
        warnings.append(f"{o.name} : {tris} triangles (> 10 000, refusé par Roblox)")
    if not has_colors and not o.material_slots:
        warnings.append(f"{o.name} : ni couleurs de sommets ni matériau")
empties = [{"name": o.name, "parent": o.parent.name if o.parent else None, "pos": r3(o.matrix_world.translation)}
           for o in bpy.data.objects if o.type == "EMPTY"]
lo, hi = world_bbox(meshes) if meshes else (Vector(), Vector())
actions = [{"name": a.name, "frames": [int(a.frame_range[0]), int(a.frame_range[1])]} for a in bpy.data.actions]
report = {
    "file": str(out_path),
    "triangles": total,
    "meshes": len(meshes),
    # Blender est Z-up ; dans le moteur : largeur = X, hauteur = Z, profondeur = Y.
    "bbox_blender": {"min": r3(lo), "max": r3(hi), "size_xyz": r3(hi - lo)},
    "srgb_fixed_accessors": fixed,
    "objects": rows,
    "empties": empties,
    "actions": actions,
    "warnings": warnings,
}


# ---------- rendus ----------

def render_views(folder, names):
    folder = Path(folder)
    folder.mkdir(parents=True, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.render.resolution_x = scene.render.resolution_y = 768
    scene.render.film_transparent = False
    scene.render.image_settings.file_format = "PNG"
    shading = scene.display.shading
    shading.light = "STUDIO"
    any_colors = any(r["colors"] for r in rows)
    for kind in (["ATTRIBUTE", "VERTEX"] if any_colors else ["MATERIAL"]):
        try:
            shading.color_type = kind
            break
        except TypeError:
            continue
    shading.show_object_outline = False
    shading.show_backface_culling = True  # comme Roblox : sinon le contour (inverted hull) masque le modèle
    shading.show_cavity = True
    for mat in bpy.data.materials:  # le mode MATERIAL lit diffuse_color, pas le nœud
        try:
            bsdf = mat.node_tree.nodes.get("Principled BSDF") if mat.use_nodes else None
            if bsdf:
                mat.diffuse_color = bsdf.inputs["Base Color"].default_value
        except Exception:
            pass
    if scene.world is None:
        scene.world = bpy.data.worlds.new("LumenWorld")
    scene.world.color = (0.93, 0.93, 0.95)

    center = (lo + hi) / 2
    size = max((hi - lo).length, 0.01)
    cam_data = bpy.data.cameras.new("LumenCam")
    cam_data.type = "ORTHO"
    cam = bpy.data.objects.new("LumenCam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    cam_data.clip_end = size * 10
    directions = {
        "front": Vector((0, -1, 0)),
        "back": Vector((0, 1, 0)),
        "left": Vector((-1, 0, 0)),
        "right": Vector((1, 0, 0)),
        "threequarter": Vector((1, -1, 0.7)).normalized(),
        "top": Vector((0, -0.001, 1)).normalized(),
    }
    out = []
    for name in names:
        d = directions.get(name)
        if d is None:
            continue
        cam.location = center + d * size * 2
        cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
        span = hi - lo
        if name in ("front", "back"):
            extent = max(span.x, span.z)
        elif name in ("left", "right"):
            extent = max(span.y, span.z)
        elif name == "top":
            extent = max(span.x, span.y)
        else:
            extent = size * 0.85
        cam_data.ortho_scale = max(extent, 0.01) * 1.15
        target = folder / f"view_{name}.png"
        scene.render.filepath = str(target)
        bpy.ops.render.render(write_still=True)
        out.append(str(target))
    bpy.data.objects.remove(cam, do_unlink=True)
    return out


if opts["renders"] and meshes:
    report["renders"] = render_views(Path(opts["renders"]).resolve(), [v.strip() for v in opts["views"].split(",") if v.strip()])

if opts["report"]:
    Path(opts["report"]).write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")

print("LUMEN_BLENDER_OK", str(out_path))

# Miniatures de la banque : importe chaque modèle, le rend en 3/4 sur fond transparent, passe au suivant.
# Appelé par : blender --background --factory-startup --python lumen-blender-thumbs.py -- --jobs jobs.json
# jobs.json : [{"id": "...", "path": "modele.glb", "out": "apercu.png"}, ...]
# Sortie : une ligne par modèle, "LUMEN_THUMB_OK\t<id>\t<png>" ou "LUMEN_THUMB_FAIL\t<id>\t<raison>".
import json
import sys
from pathlib import Path

import bpy
from mathutils import Vector

argv = sys.argv
args = argv[argv.index("--") + 1 :] if "--" in argv else []
jobs_path = args[args.index("--jobs") + 1]
size = int(args[args.index("--size") + 1]) if "--size" in args else 256
jobs = json.loads(Path(jobs_path).read_text(encoding="utf-8"))


def clear():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images, bpy.data.textures,
                 bpy.data.armatures, bpy.data.actions, bpy.data.cameras, bpy.data.lights):
        for block in list(coll):
            try:
                coll.remove(block)
            except Exception:
                pass


def setup_scene():
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.render.resolution_x = scene.render.resolution_y = size
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.display.render_aa = "8"
    shading = scene.display.shading
    shading.light = "STUDIO"
    shading.show_cavity = True
    shading.show_backface_culling = True
    return scene


def base_color_image(mat):
    """Nœud image branché sur Base Color (sinon Workbench affiche la 1re texture : émission, ORM…)."""
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if not bsdf:
        return None
    stack = [link.from_node for link in bsdf.inputs["Base Color"].links]
    while stack:
        node = stack.pop()
        if node.type == "TEX_IMAGE":
            return node
        for socket in node.inputs:
            stack.extend(link.from_node for link in socket.links)
    return None


def pick_color_mode(meshes, shading):
    has_images = False
    for mat in bpy.data.materials:
        if mat.use_nodes and mat.node_tree:
            node = base_color_image(mat)
            if node is not None:
                mat.node_tree.nodes.active = node
                has_images = True
    has_colors = any(len(o.data.color_attributes) > 0 for o in meshes)
    order = ["TEXTURE"] if has_images else (["ATTRIBUTE", "VERTEX"] if has_colors else ["MATERIAL"])
    for kind in order + ["MATERIAL"]:
        try:
            shading.color_type = kind
            break
        except TypeError:
            continue
    for mat in bpy.data.materials:  # le mode MATERIAL lit diffuse_color, pas le nœud
        try:
            bsdf = mat.node_tree.nodes.get("Principled BSDF") if mat.use_nodes else None
            if bsdf:
                mat.diffuse_color = bsdf.inputs["Base Color"].default_value
        except Exception:
            pass


def import_model(path):
    ext = Path(path).suffix.lower()
    if ext in (".glb", ".gltf"):
        bpy.ops.import_scene.gltf(filepath=path)
    elif ext == ".fbx":
        bpy.ops.import_scene.fbx(filepath=path)
    elif ext == ".obj":
        bpy.ops.wm.obj_import(filepath=path)
    else:
        raise ValueError(f"format {ext} non pris en charge")


def render(scene, out):
    bpy.context.view_layer.update()
    # Seulement ce qui est rendu : pas les formes de contrôle du squelette, cachées hors scène.
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and o.visible_get() and len(o.data.polygons) > 0]
    if not meshes:
        raise ValueError("aucun mesh")
    pick_color_mode(meshes, scene.display.shading)
    lo = Vector((1e18, 1e18, 1e18))
    hi = Vector((-1e18, -1e18, -1e18))
    for o in meshes:
        for corner in o.bound_box:
            p = o.matrix_world @ Vector(corner)
            lo = Vector(map(min, lo, p))
            hi = Vector(map(max, hi, p))
    center = (lo + hi) / 2
    radius = max((hi - lo).length / 2, 1e-4)
    cam_data = bpy.data.cameras.new("LumenThumbCam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = radius * 2.1
    cam_data.clip_start = radius * 0.01
    cam_data.clip_end = radius * 20
    cam = bpy.data.objects.new("LumenThumbCam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    d = Vector((1, -1.2, 0.8)).normalized()  # 3/4 avant, légèrement au-dessus (l'avant regarde -Y)
    cam.location = center + d * radius * 4
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    scene.render.filepath = out
    bpy.ops.render.render(write_still=True)


scene = setup_scene()
for job in jobs:
    try:
        clear()
        Path(job["out"]).parent.mkdir(parents=True, exist_ok=True)
        import_model(job["path"])
        render(scene, job["out"])
        print(f"LUMEN_THUMB_OK\t{job['id']}\t{job['out']}", flush=True)
    except Exception as exc:  # un modèle cassé ne doit pas arrêter le lot
        reason = str(exc).replace("\n", " ")[:300]
        print(f"LUMEN_THUMB_FAIL\t{job['id']}\t{reason}", flush=True)

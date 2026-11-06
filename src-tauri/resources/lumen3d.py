"""Aides Lumen pour construire des modèles low poly dans Blender (scripts lancés par `lumen-asset draft`).

Unités : 1 unité Blender = 1 stud Roblox (ou 1 m pour UEFN/Unreal, voir le skill).
Repère Blender : Z en haut, l'avant du modèle regarde vers -Y.

    from lumen3d import *
    body = box("Body", (4, 6, 3), (0, 0, 4), "#3A7BD5", bevel=0.3)
    head = rock("Head", 1.6, (0, -3.5, 6), "#3A7BD5", seed=2)
    set_pivot(head, (0, -2.8, 5.6))      # pivot au cou
    parent(head, body)
    outline(body); outline(head)
"""
import math
import random

import bmesh
import bpy
from mathutils import Matrix, Vector

__all__ = [
    "hexcol", "paint", "paint_faces", "box", "prism", "cone", "rock", "crystal", "wedge",
    "outline", "set_pivot", "parent", "empty", "flat", "join", "glow", "mirror_x", "tris",
]


def hexcol(value):
    """'#RRGGBB' -> (r, g, b, 1) en sRGB 0..1."""
    h = value.lstrip("#")
    return (int(h[0:2], 16) / 255, int(h[2:4], 16) / 255, int(h[4:6], 16) / 255, 1.0)


def _sync():
    bpy.context.view_layer.update()  # matrix_world n'est recalculé qu'à la mise à jour de la scène


def _link(obj):
    bpy.context.scene.collection.objects.link(obj)
    return obj


def _from_bmesh(name, bm):
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    obj = _link(bpy.data.objects.new(name, mesh))
    flat(obj)
    return obj


def _colors(obj):
    mesh = obj.data
    attr = mesh.color_attributes.get("Col")
    if attr is None:
        attr = mesh.color_attributes.new("Col", "BYTE_COLOR", "CORNER")
        mesh.color_attributes.active_color = attr
        try:
            mesh.color_attributes.render_color_index = mesh.color_attributes.active_color_index
        except Exception:
            pass
    return attr


def paint(obj, color, light=None):
    """Couleur de sommets unie. light='#hex' : faces tournées vers le haut plus claires (facettes lisibles)."""
    attr = _colors(obj)
    base, top = hexcol(color), hexcol(light) if light else None
    mesh = obj.data
    for poly in mesh.polygons:
        c = top if top and poly.normal.z > 0.55 else base
        for li in poly.loop_indices:
            attr.data[li].color_srgb = c
    return obj


def paint_faces(obj, color, where):
    """Peint les faces pour lesquelles where(poly) est vrai (poly.normal, poly.center en local)."""
    attr = _colors(obj)
    c = hexcol(color)
    for poly in obj.data.polygons:
        if where(poly):
            for li in poly.loop_indices:
                attr.data[li].color_srgb = c
    return obj


def flat(obj):
    for poly in obj.data.polygons:
        poly.use_smooth = False
    return obj


def box(name, size, loc=(0, 0, 0), color="#CCCCCC", bevel=0.0, light=None):
    """Boîte de taille (x, y, z) centrée sur loc. bevel > 0 : arêtes chanfreinées (1 segment, reste low poly)."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=Vector(size), verts=bm.verts)
    if bevel > 0:
        bmesh.ops.bevel(bm, geom=list(bm.edges), offset=bevel, segments=1, affect="EDGES")
    obj = _from_bmesh(name, bm)
    obj.location = loc
    return paint(obj, color, light)


def prism(name, radius, height, sides=6, loc=(0, 0, 0), color="#CCCCCC", top_scale=1.0, light=None):
    """Prisme / tronc à `sides` faces (pas de cylindre lisse). top_scale < 1 : se resserre vers le haut."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=sides, radius1=radius, radius2=radius * top_scale, depth=height)
    obj = _from_bmesh(name, bm)
    obj.location = loc
    return paint(obj, color, light)


def cone(name, radius, height, sides=5, loc=(0, 0, 0), color="#CCCCCC", light=None):
    """Pointe / corne / pic à facettes."""
    return prism(name, radius, height, sides, loc, color, top_scale=0.0, light=light)


def rock(name, radius, loc=(0, 0, 0), color="#888888", seed=0, jitter=0.25, squash=(1, 1, 1), light=None):
    """Forme ronde rendue facettée (icosphère grossière déformée) : tête, rocher, poing. Jamais une sphère lisse."""
    rnd = random.Random(seed)
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=1, radius=radius)
    for v in bm.verts:
        v.co *= 1 + rnd.uniform(-jitter, jitter)
        v.co.x *= squash[0]
        v.co.y *= squash[1]
        v.co.z *= squash[2]
    obj = _from_bmesh(name, bm)
    obj.location = loc
    return paint(obj, color, light)


def crystal(name, radius, height, loc=(0, 0, 0), color="#7FDBFF", sides=6, tip=0.35, tilt=(0, 0), light=None):
    """Cristal : prisme + pointe. tilt = (rot X, rot Y) en degrés."""
    bm = bmesh.new()
    body = height * (1 - tip)
    bmesh.ops.create_cone(bm, cap_ends=True, segments=sides, radius1=radius * 0.8, radius2=radius, depth=body)
    top = [v for v in bm.verts if v.co.z > 0]
    peak = bm.verts.new((0, 0, body / 2 + height * tip))
    ring = sorted(top, key=lambda v: math.atan2(v.co.y, v.co.x))
    cap = [f for f in bm.faces if all(v in top for v in f.verts)]
    bmesh.ops.delete(bm, geom=cap, context="FACES_ONLY")
    for a, b in zip(ring, ring[1:] + ring[:1]):
        bm.faces.new((a, b, peak))
    bmesh.ops.translate(bm, vec=(0, 0, body / 2), verts=bm.verts)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    obj = _from_bmesh(name, bm)
    obj.location = loc
    obj.rotation_euler = (math.radians(tilt[0]), math.radians(tilt[1]), 0)
    return paint(obj, color, light)


def wedge(name, size, loc=(0, 0, 0), color="#CCCCCC", light=None):
    """Coin / rampe : boîte dont l'arête haute avant est rabattue (griffes, plaques, mâchoires)."""
    obj = box(name, size, loc, color, light=light)
    for v in obj.data.vertices:
        if v.co.z > 0 and v.co.y < 0:
            v.co.z = -size[2] / 2
    obj.data.update()
    return obj


def outline(obj, thickness=0.15, color="#000000"):
    """Contour cartoon en géométrie (inverted hull) : copie gonflée le long des normales, faces retournées,
    noire, enfant de obj, nommée <obj>_Outline. Fonctionne dans Roblox (pas un shader)."""
    _sync()
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.normal_update()
    normals = {v: v.normal.copy() for v in bm.verts}
    for v in bm.verts:
        v.co += normals[v] * thickness
    bmesh.ops.reverse_faces(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(obj.name + "_Outline")
    bm.to_mesh(mesh)
    bm.free()
    hull = _link(bpy.data.objects.new(obj.name + "_Outline", mesh))
    hull.matrix_world = obj.matrix_world.copy()
    flat(hull)
    paint(hull, color)
    parent(hull, obj)
    return hull


def set_pivot(obj, point):
    """Place l'origine (pivot) de obj sur un point du monde (articulation), sans bouger la géométrie."""
    _sync()
    point = Vector(point)
    local = obj.matrix_world.inverted() @ point
    obj.data.transform(Matrix.Translation(-local))
    obj.matrix_world = obj.matrix_world @ Matrix.Translation(local)
    return obj


def parent(child, parent_obj):
    """child suit parent_obj sans bouger."""
    _sync()
    world = child.matrix_world.copy()
    child.parent = parent_obj
    child.matrix_parent_inverse = parent_obj.matrix_world.inverted()
    child.matrix_world = world
    return child


def empty(name, loc=(0, 0, 0), parent_obj=None):
    """Repère vide (LevelCenter, EntranceCenter, point d'attache…) exporté comme nœud."""
    obj = _link(bpy.data.objects.new(name, None))
    obj.location = loc
    if parent_obj:
        parent(obj, parent_obj)
    return obj


def join(objs, name):
    """Fusionne plusieurs objets en un seul mesh nommé (ex. tous les rochers -> 'Rocks')."""
    _sync()
    objs = [o for o in objs if o]
    bm = bmesh.new()
    for o in objs:
        tmp = o.data.copy()
        tmp.transform(o.matrix_world)
        bm.from_mesh(tmp)
        bpy.data.meshes.remove(tmp)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    for o in objs:
        bpy.data.objects.remove(o, do_unlink=True)
    return flat(_link(bpy.data.objects.new(name, mesh)))


def glow(obj):
    """Marque une pièce lumineuse (exportée en extras glTF `lumen_glow`). Côté moteur : Material Neon / émissif."""
    obj["lumen_glow"] = True
    return obj


def mirror_x(obj, name=None):
    """Copie symétrique gauche/droite (X -> -X) : pattes, bras, cornes. Renommer L/R."""
    _sync()
    copy = obj.copy()
    copy.data = obj.data.copy()
    copy.name = name or obj.name.replace("L", "R", 1)
    _link(copy)
    copy.data.transform(Matrix.Scale(-1, 4, (1, 0, 0)))
    copy.data.flip_normals()
    copy.location.x = -obj.location.x
    return copy


def tris(obj):
    return sum(len(p.vertices) - 2 for p in obj.data.polygons)

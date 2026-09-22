# Lanceur Lumen : exécute un script bpy puis exporte un GLB.
# Appelé par : blender --background --python lumen-blender-run.py -- --script X.py --out Y.glb
import sys
from pathlib import Path

argv = sys.argv
args = argv[argv.index("--") + 1 :] if "--" in argv else argv[1:]
script = None
out = None
i = 0
while i < len(args):
    if args[i] == "--script" and i + 1 < len(args):
        script = args[i + 1]
        i += 2
        continue
    if args[i] == "--out" and i + 1 < len(args):
        out = args[i + 1]
        i += 2
        continue
    i += 1

if not script or not out:
    raise SystemExit("Lumen blender-run : --script et --out requis")

import bpy

for obj in list(bpy.data.objects):
    bpy.data.objects.remove(obj, do_unlink=True)
for mesh in list(bpy.data.meshes):
    bpy.data.meshes.remove(mesh)
for mat in list(bpy.data.materials):
    bpy.data.materials.remove(mat)

script_path = Path(script).resolve()
source = script_path.read_text(encoding="utf-8")
ns = {"__name__": "__main__", "__file__": str(script_path)}
exec(compile(source, str(script_path), "exec"), ns)

out_path = Path(out).resolve()
out_path.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.export_scene.gltf(
    filepath=str(out_path),
    export_format="GLB",
    export_apply=True,
    export_yup=True,
)
print("LUMEN_BLENDER_OK", str(out_path))

# Écrit par Lumen. Toolset MCP du projet : ce que les outils d'Epic ne savent pas faire.
# Chargé au démarrage de l'éditeur par Content/Python/init_unreal.py.

import io
import os
import traceback
from contextlib import redirect_stdout

import unreal

import toolset_registry
from toolset_registry.registration import Registration


def _project_file(path: str) -> str:
    """Chemin absolu : les chemins relatifs partent du dossier du projet (ex. assets/meshes/SM_Coffre.glb)."""
    if os.path.isabs(path):
        return os.path.normpath(path)
    root = unreal.Paths.convert_relative_path_to_full(unreal.Paths.project_dir())
    return os.path.normpath(os.path.join(root, path))


MESH_EXTENSIONS = (".glb", ".gltf", ".fbx", ".obj")
_TMP_FOLDER = "/Game/__LumenTmp"


def _import_mesh(source: str, folder_path: str, asset_name: str) -> list[str]:
    """Un modèle de la banque contient souvent des dizaines de morceaux : Interchange les fusionne
    en un seul Static Mesh, rangé avec ses matériaux dans folder_path/asset_name/."""
    eal = unreal.EditorAssetLibrary
    folder = f"{folder_path}/{asset_name}"
    if eal.does_directory_exist(folder) and eal.list_assets(folder, recursive=True):
        raise RuntimeError(f"{folder} existe déjà : choisis un autre nom, ou supprime l'ancien.")
    base = unreal.load_asset("/Interchange/Pipelines/DefaultAssetsPipeline")
    eal.delete_directory(_TMP_FOLDER)
    pipeline = eal.duplicate_loaded_asset(base, f"{_TMP_FOLDER}/CombinePipeline")
    try:
        pipeline.get_editor_property("mesh_pipeline").set_editor_property(
            "combine_static_meshes_behavior", unreal.InterchangeCombineStaticMeshesBehavior.VISIBLE_ONLY
        )
        params = unreal.ImportAssetParameters()
        params.is_automated = True
        params.override_pipelines.append(unreal.SoftObjectPath(pipeline.get_path_name()))
        manager = unreal.InterchangeManager.get_interchange_manager_scripted()
        manager.import_asset(folder, unreal.InterchangeManager.create_source_data(source), params)
    finally:
        eal.delete_directory(_TMP_FOLDER)
    paths = [str(p) for p in eal.list_assets(folder, recursive=True)]
    if not paths:
        raise RuntimeError(f"Unreal n'a rien importé depuis {source} (format refusé ?). Regarde le journal (LogsToolset).")
    # Le mesh fusionné prend le nom du fichier : on lui donne le nom demandé.
    meshes = [p for p in paths if isinstance(unreal.load_asset(p), unreal.StaticMesh)]
    if len(meshes) == 1:
        target = f"{folder}/{asset_name}"
        if eal.rename_asset(meshes[0], target):
            paths = [str(p) for p in eal.list_assets(folder, recursive=True)]
    eal.save_directory(folder)
    return paths


@unreal.uclass()
class LumenTools(unreal.ToolsetDefinition):
    """Outils Lumen : importer n'importe quel fichier de la banque (glb, gltf, fbx, obj, png, wav...) et exécuter du Python dans l'éditeur."""

    @toolset_registry.tool_call
    @staticmethod
    def import_file(source_file: str, folder_path: str, asset_name: str) -> list[str]:
        """Importe un fichier du disque dans le Content Browser, par Interchange (accepte .glb / .gltf, contrairement à StaticMeshTools.import_file).

        Args:
            source_file: Chemin du fichier, absolu ou relatif au projet (ex. assets/meshes/SM_Coffre_VS_0124.glb).
            folder_path: Dossier de destination (ex. /Game/Lumen/Meshes).
            asset_name: Nom de l'asset créé (ex. SM_Coffre). Les assets secondaires (matériaux, textures) en dérivent.

        Returns:
            Les chemins des assets créés.
        """
        source = _project_file(source_file)
        if not os.path.isfile(source):
            raise FileNotFoundError(f"Fichier introuvable : {source}")
        folder_path = folder_path.rstrip("/")
        if os.path.splitext(source)[1].lower() in MESH_EXTENSIONS:
            return _import_mesh(source, folder_path, asset_name)
        if unreal.EditorAssetLibrary.does_asset_exist(f"{folder_path}/{asset_name}"):
            raise RuntimeError(f"{folder_path}/{asset_name} existe déjà : choisis un autre nom, ou supprime l'ancien.")
        task = unreal.AssetImportTask()
        task.filename = source
        task.destination_path = folder_path
        task.destination_name = asset_name
        task.replace_existing = False
        task.automated = True
        task.save = True
        unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])
        paths = [str(p) for p in task.get_editor_property("imported_object_paths")]
        if not paths:
            raise RuntimeError(f"Unreal n'a rien importé depuis {source} (format refusé ?). Regarde le journal (LogsToolset).")
        return paths

    @toolset_registry.tool_call
    @staticmethod
    def execute_python(code: str) -> str:
        """Exécute du Python dans l'éditeur, avec le module `unreal` complet. Pour ce qu'aucun autre outil ne couvre.

        Args:
            code: Code Python. Ce qui est affiché par print() est renvoyé. Mets le résultat dans la variable `result` pour le renvoyer aussi.

        Returns:
            La sortie de print(), puis la valeur de `result` si elle est définie.
        """
        scope = {"unreal": unreal, "__name__": "__lumen__"}
        out = io.StringIO()
        try:
            with redirect_stdout(out):
                exec(code, scope)
        except Exception:
            return out.getvalue() + "\nErreur :\n" + traceback.format_exc()
        text = out.getvalue()
        if "result" in scope:
            text += f"\nresult = {scope['result']!r}"
        return text.strip() or "(aucune sortie)"


_registration = Registration([LumenTools])
_registration.register()

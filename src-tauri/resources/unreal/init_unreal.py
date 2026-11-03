# Écrit par Lumen. Unreal exécute ce fichier à l'ouverture du projet (plugin Python activé).
try:
    import lumen_toolset  # noqa: F401 — enregistre le toolset MCP « LumenTools »
except Exception as error:  # le projet doit s'ouvrir même si le MCP est désactivé
    import unreal

    unreal.log_warning(f"Lumen : toolset MCP non chargé ({error})")

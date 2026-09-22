# Lumen

Application Windows pour créer des jeux Roblox avec des agents IA (Claude, Codex, Cursor, Antigravity).

## Tester (ami)

1. Télécharge le dernier **Lumen_…_x64-setup.exe** dans [Releases](https://github.com/Hazeyy5/lumen-studio/releases).
2. Installe (pas besoin d’admin).
3. Connecte-toi à Roblox dans Lumen, puis colle tes clés dans **Réglages** (Gemini, etc.).
4. Les clés restent sur **ton** PC (`AppData/Lumen/keys.json`). Rien n’est dans le dépôt.

Windows 10/11 avec WebView2 (déjà là sur la plupart des machines).

## Mises à jour

Lumen vérifie GitHub au démarrage. Une bannière apparaît s’il y a une nouvelle version, ou **Réglages → Vérifier**.

## Développeur

```bash
npm install
npm run tauri dev
```

Nouvelle version (après bump de `version` dans `package.json` et `src-tauri/tauri.conf.json`) :

```bash
git tag v0.1.1
git push origin v0.1.1
```

GitHub Actions compile l’installeur Windows et publie la Release.

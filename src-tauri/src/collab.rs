use crate::projects::{read_project_meta, slugify, Project};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectShare {
    pub shared: bool,
    pub remote: String,
    pub dirty: bool,
    pub detail: String,
}

fn output_text(out: &Output) -> String {
    let mut text = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !stdout.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&stdout);
    }
    text
}

fn explain(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("authentication")
        || lower.contains("gh auth login")
        || lower.contains("http 401")
        || lower.contains("could not read username")
    {
        return "GitHub n’est pas connecté sur ce PC. Dans un terminal : gh auth login. Ou colle un jeton (droits repo) dans Réglages.".into();
    }
    let trimmed = raw.trim();
    if trimmed.chars().count() > 500 {
        trimmed.chars().take(500).collect()
    } else if trimmed.is_empty() {
        "La commande Git a échoué.".into()
    } else {
        trimmed.to_string()
    }
}

fn run(dir: &Path, program: &str, args: &[String]) -> Result<Output, String> {
    let exe = if program == "git" {
        crate::git_tools::git_exe()?
    } else if program == "gh" {
        crate::git_tools::gh_exe()?
    } else {
        PathBuf::from(program)
    };
    let git = crate::git_tools::git_exe()?;
    let gh = crate::git_tools::gh_exe()?;
    let mut path = String::new();
    if let Some(parent) = git.parent() {
        path.push_str(&parent.display().to_string());
        path.push(';');
    }
    if let Some(parent) = gh.parent() {
        path.push_str(&parent.display().to_string());
        path.push(';');
    }
    if let Ok(current) = std::env::var("PATH") {
        path.push_str(&current);
    }
    let mut cmd = Command::new(exe);
    cmd.args(args).current_dir(dir).env("PATH", path);
    if let Some(token) = crate::sync::github_token() {
        cmd.env("GH_TOKEN", &token);
        cmd.env("GITHUB_TOKEN", &token);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.output()
        .map_err(|e| format!("{program} n’a pas pu démarrer ({e})"))
}

fn git(dir: &Path, args: &[String]) -> Result<Output, String> {
    run(dir, "git", args)
}

fn git_ok(dir: &Path, args: &[String]) -> Result<String, String> {
    let out = git(dir, args)?;
    if !out.status.success() {
        return Err(explain(&output_text(&out)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_string()).collect()
}

fn github_login() -> Result<String, String> {
    let out = run(Path::new("."), "gh", &strings(&["api", "user"]))?;
    if !out.status.success() {
        return Err(explain(&output_text(&out)));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    value["login"]
        .as_str()
        .map(|login| login.to_string())
        .ok_or_else(|| "Compte GitHub introuvable.".into())
}

fn git_identity() -> Result<(String, String), String> {
    let login = github_login()?;
    Ok((login.clone(), format!("{login}@users.noreply.github.com")))
}

fn parse_github_repo(raw: &str) -> Option<(String, String)> {
    let text = raw.trim().trim_end_matches('/').trim_end_matches(".git");
    let rest = if let Some(rest) = text.strip_prefix("git@github.com:") {
        rest
    } else if let Some(index) = text.find("github.com/") {
        &text[index + "github.com/".len()..]
    } else {
        text
    };
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

fn origin(dir: &Path) -> Option<String> {
    git_ok(dir, &strings(&["remote", "get-url", "origin"])).ok()
}

fn remote_slug(dir: &Path) -> Option<String> {
    let url = origin(dir)?;
    let (owner, repo) = parse_github_repo(&url)?;
    Some(format!("{owner}/{repo}"))
}

fn dirty(dir: &Path) -> bool {
    git_ok(dir, &strings(&["status", "--porcelain"]))
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}

fn ensure_gitignore(dir: &Path) -> Result<(), String> {
    let path = dir.join(".gitignore");
    let mut text = fs::read_to_string(&path).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for line in ["node_modules", "out", "include", ".lumen-swarm.json", ".lumen.json"] {
        if !text.lines().any(|existing| existing.trim() == line) {
            text.push_str(line);
            text.push('\n');
        }
    }
    fs::write(path, text).map_err(|e| e.to_string())
}

fn commit_changes(dir: &Path, message: &str) -> Result<bool, String> {
    let pending = git_ok(dir, &strings(&["status", "--porcelain"]))?;
    if pending.trim().is_empty() {
        return Ok(false);
    }
    git_ok(dir, &strings(&["add", "-A"]))?;
    let still = git_ok(dir, &strings(&["status", "--porcelain"]))?;
    if still.trim().is_empty() {
        return Ok(false);
    }
    let (name, email) = git_identity()?;
    let args = vec![
        "-c".into(),
        format!("user.name={name}"),
        "-c".into(),
        format!("user.email={email}"),
        "commit".into(),
        "-m".into(),
        message.into(),
    ];
    let out = git(dir, &args)?;
    if !out.status.success() {
        return Err(explain(&output_text(&out)));
    }
    Ok(true)
}

fn current_branch(dir: &Path) -> Result<String, String> {
    let name = git_ok(dir, &strings(&["rev-parse", "--abbrev-ref", "HEAD"]))?;
    if name.is_empty() || name == "HEAD" {
        return Err("Branche Git introuvable.".into());
    }
    Ok(name)
}

fn push_head(dir: &Path) -> Result<(), String> {
    let branch = current_branch(dir)?;
    let out = git(dir, &strings(&["push", "-u", "origin", &branch]))?;
    if out.status.success() {
        return Ok(());
    }
    let pulled = git(
        dir,
        &strings(&["pull", "--rebase", "origin", &branch]),
    )?;
    if !pulled.status.success() {
        let _ = git(dir, &strings(&["rebase", "--abort"]));
        return Err(explain(&format!(
            "L’autre a envoyé des changements entre-temps. Reçois d’abord, puis renvoie.\n{}",
            output_text(&pulled)
        )));
    }
    let again = git(dir, &strings(&["push", "-u", "origin", &branch]))?;
    if again.status.success() {
        Ok(())
    } else {
        Err(explain(&output_text(&again)))
    }
}

fn invite(owner_repo: &str, friend: &str) -> Result<(), String> {
    let friend = friend.trim().trim_start_matches('@');
    if friend.is_empty() {
        return Ok(());
    }
    if !friend
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
        || friend.len() > 39
    {
        return Err("Pseudo GitHub invalide.".into());
    }
    let out = run(
        Path::new("."),
        "gh",
        &strings(&[
            "api",
            "--method",
            "PUT",
            &format!("repos/{owner_repo}/collaborators/{friend}"),
            "-f",
            "permission=push",
        ]),
    )?;
    if out.status.success() {
        Ok(())
    } else {
        Err(explain(&output_text(&out)))
    }
}

fn status_of(dir: &Path, extra: &str) -> ProjectShare {
    let remote = remote_slug(dir).unwrap_or_default();
    let shared = !remote.is_empty();
    let dirty = dir.join(".git").is_dir() && dirty(dir);
    let detail = if !extra.is_empty() {
        extra.to_string()
    } else if !shared {
        "Pas encore partagé.".into()
    } else if dirty {
        format!("{remote} · des fichiers ont changé, prêts à envoyer.")
    } else {
        format!("{remote} · à jour.")
    };
    ProjectShare {
        shared,
        remote,
        dirty,
        detail,
    }
}

fn project_dir(path: &str) -> Result<PathBuf, String> {
    let dir = PathBuf::from(path);
    if !dir.join(".lumen.json").is_file() {
        return Err("Ce dossier n’est pas un projet Lumen.".into());
    }
    Ok(dir)
}

#[tauri::command(async)]
pub fn project_share_status(project_path: String) -> Result<ProjectShare, String> {
    let dir = project_dir(&project_path)?;
    Ok(status_of(&dir, ""))
}

#[tauri::command(async)]
pub fn share_project(project_path: String, friend: Option<String>) -> Result<ProjectShare, String> {
    crate::git_tools::ensure_github_login()?;
    let dir = project_dir(&project_path)?;
    let project = read_project_meta(&project_path)?;
    ensure_gitignore(&dir)?;
    if !dir.join(".git").is_dir() {
        git_ok(&dir, &strings(&["init", "-b", "main"]))?;
    }
    let first = git_ok(&dir, &strings(&["rev-parse", "HEAD"])).is_err();
    commit_changes(
        &dir,
        if first {
            "Partage du projet Lumen."
        } else {
            "Mise à jour depuis Lumen."
        },
    )?;
    if origin(&dir).is_none() {
        let mut name = slugify(&project.name);
        if name.is_empty() {
            name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("jeu")
                .to_string();
        }
        if !name.starts_with("lumen-") {
            name = format!("lumen-{name}");
        }
        let out = run(
            &dir,
            "gh",
            &strings(&[
                "repo",
                "create",
                &name,
                "--private",
                "--source",
                ".",
                "--remote",
                "origin",
                "--push",
                "--description",
                "Projet Lumen partagé",
            ]),
        )?;
        if !out.status.success() {
            return Err(explain(&output_text(&out)));
        }
    } else {
        push_head(&dir)?;
    }
    let slug = remote_slug(&dir).unwrap_or_default();
    let mut note = format!("Partagé : {slug}. Ton ami colle ce nom dans Rejoindre.");
    if let Some(friend) = friend.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        match invite(&slug, friend) {
            Ok(()) => note = format!("{note} Invitation envoyée à {friend}."),
            Err(err) => note = format!("{note} Invitation non envoyée : {err}"),
        }
    }
    Ok(status_of(&dir, &note))
}

#[tauri::command(async)]
pub fn push_project(project_path: String) -> Result<ProjectShare, String> {
    crate::git_tools::ensure_github_login()?;
    let dir = project_dir(&project_path)?;
    if origin(&dir).is_none() {
        return Err("Partage d’abord le projet.".into());
    }
    ensure_gitignore(&dir)?;
    let wrote = commit_changes(&dir, "Mise à jour depuis Lumen.")?;
    push_head(&dir)?;
    let note = if wrote {
        "Envoyé."
    } else {
        "Rien de neuf à envoyer."
    };
    Ok(status_of(&dir, note))
}

#[tauri::command(async)]
pub fn pull_project(project_path: String) -> Result<ProjectShare, String> {
    crate::git_tools::ensure_github_login()?;
    let dir = project_dir(&project_path)?;
    if origin(&dir).is_none() {
        return Err("Ce projet n’est pas partagé.".into());
    }
    let branch = current_branch(&dir)?;
    let out = git(&dir, &strings(&["pull", "--rebase", "origin", &branch]))?;
    if !out.status.success() {
        let _ = git(&dir, &strings(&["rebase", "--abort"]));
        return Err(explain(&format!(
            "Impossible de recevoir. {}\nUn seul des deux corrige, puis envoie.",
            output_text(&out)
        )));
    }
    Ok(status_of(&dir, "Reçu. Relance la sync Studio si tu es celui qui envoie vers le jeu."))
}

#[tauri::command(async)]
pub fn join_project(repo: String) -> Result<Project, String> {
    crate::git_tools::ensure_github_login()?;
    let (owner, name) = parse_github_repo(&repo).ok_or("Indique le projet ainsi : pseudo/lumen-nom")?;
    let dest_name = slugify(&name);
    if dest_name.is_empty() {
        return Err("Nom de dépôt invalide.".into());
    }
    let dest = crate::projects::projects_root()?.join(&dest_name);
    if dest.exists() {
        return Err("Ce projet est déjà sur ce PC.".into());
    }
    let parent = dest
        .parent()
        .ok_or("Dossier des projets introuvable")?;
    let out = run(
        parent,
        "gh",
        &strings(&["repo", "clone", &format!("{owner}/{name}"), &dest_name]),
    )?;
    if !out.status.success() {
        let _ = fs::remove_dir_all(&dest);
        return Err(explain(&output_text(&out)));
    }
    let meta = dest.join(".lumen.json");
    if !meta.is_file() {
        let display = name
            .trim_start_matches("lumen-")
            .replace('-', " ");
        let project = Project {
            name: if display.trim().is_empty() {
                name.clone()
            } else {
                display
            },
            path: dest.to_string_lossy().into(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|_| "0".into()),
            bound_place_name: None,
            bound_place_id: None,
            reference_paths: Vec::new(),
            reference_path: None,
        };
        fs::write(
            &meta,
            serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    read_project_meta(&dest.to_string_lossy())
}

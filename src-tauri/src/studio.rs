use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::projects::{bind_project_place, read_project_meta};

const PORT: u16 = 17420;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StudioHeartbeat {
    pub connected: bool,
    pub place_name: String,
    pub place_id: i64,
    pub last_seen: u64,
    pub plugin_installed: bool,
    #[serde(default)]
    pub bound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceOffer {
    pub serving: bool,
    pub port: u16,
    pub project_name: String,
    pub project_path: String,
    pub bound_place_name: String,
    pub bound_place_id: i64,
    pub generation: u64,
    #[serde(default)]
    pub want_textures: bool,
}

impl Default for PlaceOffer {
    fn default() -> Self {
        Self {
            serving: false,
            port: 34873,
            project_name: String::new(),
            project_path: String::new(),
            bound_place_name: String::new(),
            bound_place_id: 0,
            generation: 0,
            want_textures: false,
        }
    }
}

pub type StudioState = Arc<Mutex<StudioHeartbeat>>;
pub type OfferState = Arc<Mutex<PlaceOffer>>;

fn plugins_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("Roblox").join("Plugins"))
}

fn plugin_path() -> Option<PathBuf> {
    plugins_dir().map(|p| p.join("LumenSync.rbxmx"))
}

pub fn plugin_installed() -> bool {
    plugin_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn set_offer_from_project(offer: &OfferState, project_path: &str, serving: bool, port: u16) {
    let meta = read_project_meta(project_path).ok();
    let mut guard = offer.lock().unwrap();
    guard.serving = serving;
    guard.port = port;
    guard.project_path = project_path.to_string();
    if let Some(project) = meta {
        guard.project_name = project.name;
        guard.bound_place_name = project.bound_place_name.unwrap_or_default();
        guard.bound_place_id = project.bound_place_id.unwrap_or(0);
    }
    if serving {
        guard.generation = guard.generation.saturating_add(1);
    }
}

pub fn clear_offer(offer: &OfferState) {
    let mut guard = offer.lock().unwrap();
    guard.serving = false;
}

fn json_ok(body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
    )
}

pub fn spawn_server(app: AppHandle, state: StudioState, offer: OfferState) {
    let _ = install_studio_plugin();
    thread::spawn(move || {
        let server = match tiny_http::Server::http(format!("127.0.0.1:{PORT}")) {
            Ok(s) => s,
            Err(_) => return,
        };
        for mut request in server.incoming_requests() {
            let url = request.url().split('?').next().unwrap_or("/").to_string();
            let method = request.method().clone();
            let mut body = String::new();
            let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);

            if url == "/offer" && method == tiny_http::Method::Get {
                let mut snap = offer.lock().unwrap().clone();
                snap.want_textures = crate::textures::want_studio_scan();
                let _ = request.respond(json_ok(serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into())));
                continue;
            }

            if url == "/textures" && method == tiny_http::Method::Post {
                let payload: serde_json::Value =
                    serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
                let added = crate::textures::ingest_studio_textures(&payload);
                let _ = app.emit("textures-imported", added);
                let _ = request.respond(json_ok(format!("{{\"ok\":true,\"added\":{added}}}")));
                continue;
            }

            if url == "/bind" && method == tiny_http::Method::Post {
                let payload: serde_json::Value =
                    serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
                let incoming_name = payload
                    .get("placeName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Place")
                    .to_string();
                let place_id = payload.get("placeId").and_then(|v| v.as_i64()).unwrap_or(0);
                let (bound_name, bound_id, project_path) = {
                    let guard = offer.lock().unwrap();
                    (
                        guard.bound_place_name.clone(),
                        guard.bound_place_id,
                        guard.project_path.clone(),
                    )
                };
                let prev = state.lock().unwrap().clone();
                let place_name = resolve_incoming_place_name(
                    incoming_name,
                    place_id,
                    &prev,
                    &bound_name,
                    bound_id,
                );
                let result = if project_path.is_empty() {
                    Err("Aucun projet Lumen en sync".to_string())
                } else {
                    bind_project_place(&project_path, &place_name, place_id)
                };
                match result {
                    Ok(project) => {
                        {
                            let mut guard = offer.lock().unwrap();
                            guard.bound_place_name = place_name.clone();
                            guard.bound_place_id = place_id;
                            guard.project_name = project.name.clone();
                        }
                        {
                            let mut guard = state.lock().unwrap();
                            guard.connected = true;
                            guard.place_name = place_name;
                            guard.place_id = place_id;
                            guard.last_seen = now_secs();
                            guard.plugin_installed = true;
                            guard.bound = true;
                            let snap = guard.clone();
                            drop(guard);
                            let _ = app.emit("studio-heartbeat", snap);
                            let _ = app.emit("place-bound", project);
                        }
                        let _ = request.respond(json_ok("{\"ok\":true}".into()));
                    }
                    Err(err) => {
                        let _ = request.respond(
                            json_ok(format!("{{\"ok\":false,\"error\":{}}}", serde_json::to_string(&err).unwrap_or_else(|_| "\"erreur\"".into()))),
                        );
                    }
                }
                continue;
            }

            if url.starts_with("/studio") && method == tiny_http::Method::Post {
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&body) {
                    let incoming_name = payload
                        .get("placeName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Place")
                        .to_string();
                    let place_id = payload.get("placeId").and_then(|v| v.as_i64()).unwrap_or(0);
                    let (bound_name, bound_id) = {
                        let offer = offer.lock().unwrap();
                        (offer.bound_place_name.clone(), offer.bound_place_id)
                    };
                    let prev = state.lock().unwrap().clone();
                    let place_name = resolve_incoming_place_name(
                        incoming_name,
                        place_id,
                        &prev,
                        &bound_name,
                        bound_id,
                    );
                    let bound = (!bound_name.is_empty() && bound_name == place_name)
                        || (bound_id != 0 && bound_id == place_id);
                    let mut guard = state.lock().unwrap();
                    guard.connected = true;
                    guard.place_name = place_name;
                    guard.place_id = place_id;
                    guard.last_seen = now_secs();
                    guard.plugin_installed = true;
                    guard.bound = bound;
                    let snap = guard.clone();
                    drop(guard);
                    let _ = app.emit("studio-heartbeat", snap);
                }
                let _ = request.respond(json_ok("{\"ok\":true}".into()));
                continue;
            }

            let _ = request.respond(tiny_http::Response::from_string("lumen"));
        }
    });
}

fn is_generic_place_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "" | "Game" | "Place" | "Workspace" | "DataModel"
    )
}

fn resolve_incoming_place_name(
    incoming: String,
    place_id: i64,
    prev: &StudioHeartbeat,
    bound_name: &str,
    bound_id: i64,
) -> String {
    if !is_generic_place_name(&incoming) {
        return incoming;
    }
    if place_id != 0 && prev.place_id == place_id && !is_generic_place_name(&prev.place_name) {
        return prev.place_name.clone();
    }
    if place_id != 0
        && bound_id == place_id
        && !bound_name.is_empty()
        && !is_generic_place_name(bound_name)
    {
        return bound_name.to_string();
    }
    if !bound_name.is_empty() && !is_generic_place_name(bound_name) {
        return bound_name.to_string();
    }
    incoming
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
pub fn studio_status(state: tauri::State<StudioState>) -> StudioHeartbeat {
    let mut guard = state.lock().unwrap();
    guard.plugin_installed = plugin_installed();
    if guard.connected && now_secs().saturating_sub(guard.last_seen) > 6 {
        guard.connected = false;
        guard.bound = false;
    }
    guard.clone()
}

#[tauri::command]
pub fn sync_offer(offer: tauri::State<OfferState>) -> PlaceOffer {
    offer.lock().unwrap().clone()
}

#[tauri::command]
pub fn remind_studio(offer: tauri::State<OfferState>) -> PlaceOffer {
    let mut guard = offer.lock().unwrap();
    if guard.serving {
        guard.generation = guard.generation.saturating_add(1);
    }
    guard.clone()
}

#[tauri::command]
pub fn bind_open_place(
    offer: tauri::State<OfferState>,
    state: tauri::State<StudioState>,
) -> Result<crate::projects::Project, String> {
    let snap = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        if !guard.connected {
            return Err("Studio n’est pas connecté. Ouvre une place, le plugin Lumen envoie le signal.".into());
        }
        guard.clone()
    };
    let (bound_name, bound_id, project_path) = {
        let guard = offer.lock().map_err(|e| e.to_string())?;
        (
            guard.bound_place_name.clone(),
            guard.bound_place_id,
            guard.project_path.clone(),
        )
    };
    if project_path.is_empty() {
        return Err("Aucun projet Lumen en sync".into());
    }
    let place_name = resolve_incoming_place_name(
        snap.place_name.clone(),
        snap.place_id,
        &snap,
        &bound_name,
        bound_id,
    );
    let place_id = snap.place_id;
    let project = bind_project_place(&project_path, &place_name, place_id)?;
    {
        let mut guard = offer.lock().map_err(|e| e.to_string())?;
        guard.bound_place_name = place_name.clone();
        guard.bound_place_id = place_id;
        guard.project_name = project.name.clone();
    }
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.bound = true;
        guard.place_name = place_name;
        guard.place_id = place_id;
    }
    Ok(project)
}

#[tauri::command]
pub fn install_studio_plugin() -> Result<String, String> {
    let dir = plugins_dir().ok_or("Dossier Plugins Roblox introuvable")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("LumenSync.rbxmx");
    fs::write(&dest, plugin_rbxmx()).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into())
}

fn plugin_rbxmx() -> String {
    let source = r#"local HttpService = game:GetService("HttpService")
local HOST = "http://127.0.0.1:17420"

local toolbar = plugin:CreateToolbar("Lumen")
local button = toolbar:CreateButton("Lumen", "Lier cette place au projet Lumen ouvert", "")

local info = DockWidgetPluginGuiInfo.new(Enum.InitialDockState.Float, false, false, 440, 210, 380, 180)
local widget = plugin:CreateDockWidgetPluginGui("LumenPlaceConnect", info)
widget.Title = "Lumen"
widget.ZIndexBehavior = Enum.ZIndexBehavior.Sibling

local root = Instance.new("Frame")
root.Size = UDim2.fromScale(1, 1)
root.BackgroundColor3 = Color3.fromRGB(243, 238, 228)
root.BorderSizePixel = 0
root.Parent = widget

local pad = Instance.new("UIPadding")
pad.PaddingTop = UDim.new(0, 16)
pad.PaddingBottom = UDim.new(0, 16)
pad.PaddingLeft = UDim.new(0, 18)
pad.PaddingRight = UDim.new(0, 18)
pad.Parent = root

local title = Instance.new("TextLabel")
title.BackgroundTransparency = 1
title.Size = UDim2.new(1, 0, 0, 26)
title.Font = Enum.Font.GothamMedium
title.TextSize = 18
title.TextXAlignment = Enum.TextXAlignment.Left
title.TextColor3 = Color3.fromRGB(28, 23, 18)
title.Text = "Connecter cette place ?"
title.Parent = root

local body = Instance.new("TextLabel")
body.BackgroundTransparency = 1
body.Position = UDim2.new(0, 0, 0, 34)
body.Size = UDim2.new(1, 0, 0, 88)
body.Font = Enum.Font.Gotham
body.TextSize = 15
body.TextWrapped = true
body.TextXAlignment = Enum.TextXAlignment.Left
body.TextYAlignment = Enum.TextYAlignment.Top
body.TextColor3 = Color3.fromRGB(107, 97, 86)
body.Text = "En attente de Lumen…"
body.Parent = root

local function makeBtn(text, color, xScale)
	local b = Instance.new("TextButton")
	b.AnchorPoint = Vector2.new(0, 1)
	b.Size = UDim2.new(0, 138, 0, 34)
	b.Position = UDim2.new(xScale, 0, 1, 0)
	b.BackgroundColor3 = color
	b.TextColor3 = Color3.fromRGB(255, 250, 243)
	b.Font = Enum.Font.GothamMedium
	b.TextSize = 15
	b.Text = text
	b.AutoButtonColor = true
	b.Parent = root
	local c = Instance.new("UICorner")
	c.CornerRadius = UDim.new(0, 8)
	c.Parent = b
	return b
end

local connectBtn = makeBtn("Connecter", Color3.fromRGB(184, 92, 56), 0)
local laterBtn = makeBtn("Plus tard", Color3.fromRGB(90, 82, 74), 0)
laterBtn.Position = UDim2.new(0, 150, 1, 0)

local currentOffer = nil
local dismissedGen = tonumber(plugin:GetSetting("lumenDismissedGen")) or 0
local shownGen = -1
local connectedUi = false

local function decode(res)
	if not res or not res.Success then
		return nil
	end
	local ok, data = pcall(function()
		return HttpService:JSONDecode(res.Body)
	end)
	if ok then
		return data
	end
	return nil
end

local function jsonGet(path)
	local ok, res = pcall(function()
		return HttpService:RequestAsync({ Url = HOST .. path, Method = "GET" })
	end)
	if ok then
		return decode(res)
	end
end

local function jsonPost(path, payload)
	local ok, res = pcall(function()
		return HttpService:RequestAsync({
			Url = HOST .. path,
			Method = "POST",
			Headers = { ["Content-Type"] = "application/json" },
			Body = HttpService:JSONEncode(payload),
		})
	end)
	return ok and res and res.Success
end

local GENERIC = {
	Game = true,
	Place = true,
	Workspace = true,
	DataModel = true,
}
local cloudName = plugin:GetSetting("lumenCloudName")
local cloudPlaceId = tonumber(plugin:GetSetting("lumenCloudPlaceId")) or 0
local fetchingCloud = false

local function resolvePlaceName()
	local id = game.PlaceId
	local cached = type(cloudName) == "string"
		and cloudName ~= ""
		and cloudPlaceId == id
		and not GENERIC[cloudName]
	if id ~= 0 and not cached and not fetchingCloud then
		fetchingCloud = true
		task.spawn(function()
			local ok, info = pcall(function()
				return game:GetService("MarketplaceService"):GetProductInfo(id)
			end)
			if ok and type(info) == "table" and type(info.Name) == "string" and info.Name ~= "" and not GENERIC[info.Name] then
				cloudName = info.Name
				cloudPlaceId = id
				plugin:SetSetting("lumenCloudName", cloudName)
				plugin:SetSetting("lumenCloudPlaceId", id)
			end
			fetchingCloud = false
		end)
	end
	if cached then
		return cloudName
	end
	if game.Name ~= "" and not GENERIC[game.Name] then
		return game.Name
	end
	if type(cloudName) == "string" and cloudName ~= "" and not GENERIC[cloudName] then
		return cloudName
	end
	return game.Name
end

local function heartbeat()
	jsonPost("/studio", {
		placeName = resolvePlaceName(),
		placeId = game.PlaceId,
	})
end

local TEX_PROPS = {
	"Texture", "TextureId", "TextureID", "Image",
	"ColorMap", "MetalnessMap", "NormalMap", "RoughnessMap",
	"SkyboxBk", "SkyboxDn", "SkyboxFt", "SkyboxLf", "SkyboxRt", "SkyboxUp",
	"Graphic", "ShirtTemplate", "PantsTemplate",
}

local function isContent(s)
	if type(s) ~= "string" or s == "" then
		return false
	end
	local l = string.lower(s)
	return string.find(l, "rbxasset", 1, true) ~= nil or string.find(l, "id=", 1, true) ~= nil
end

local function scanTextures()
	local seen = {}
	local items = {}
	local function tilingOf(inst)
		local scaleType, tileSize
		pcall(function()
			scaleType = inst.ScaleType.Name
		end)
		pcall(function()
			local ts = inst.TileSize
			tileSize = {
				xScale = ts.X.Scale,
				xOffset = ts.X.Offset,
				yScale = ts.Y.Scale,
				yOffset = ts.Y.Offset,
			}
		end)
		return scaleType, tileSize
	end
	local function consider(inst, value)
		if not isContent(value) then
			return
		end
		local scaleType, tileSize = tilingOf(inst)
		if seen[value] then
			local item = items[seen[value]]
			if item and not item.scaleType and scaleType then
				item.scaleType = scaleType
				item.tileSize = tileSize
			end
			return
		end
		local name = inst.Name
		if inst.Parent and (name == "Texture" or name == "Decal" or name == "ImageLabel") then
			name = inst.Parent.Name .. "/" .. name
		end
		table.insert(items, {
			name = name,
			content = value,
			scaleType = scaleType,
			tileSize = tileSize,
		})
		seen[value] = #items
	end
	local roots = {
		workspace,
		game:GetService("Lighting"),
		game:GetService("ReplicatedStorage"),
		game:GetService("ServerStorage"),
		game:GetService("StarterGui"),
		game:GetService("StarterPack"),
		game:GetService("MaterialService"),
	}
	for _, root in ipairs(roots) do
		pcall(function()
			for _, inst in ipairs(root:GetDescendants()) do
				for _, prop in ipairs(TEX_PROPS) do
					local ok, value = pcall(function()
						return inst[prop]
					end)
					if ok then
						consider(inst, value)
					end
				end
				if #items >= 250 then
					return
				end
			end
		end)
		if #items >= 250 then
			break
		end
	end
	return items
end

local function resetButtons()
	connectedUi = false
	connectBtn.Visible = true
	connectBtn.Text = "Connecter"
	laterBtn.Text = "Plus tard"
end

local function present(offer)
	currentOffer = offer
	title.Text = "Connecter cette place ?"
	body.Text = string.format(
		"Le projet Lumen « %s » tourne sur 127.0.0.1:%s.\nLier cette place (« %s ») ? Ensuite, dans le plugin Rojo, utilise ce port — pas 34872 (VibeStarter).",
		tostring(offer.projectName or "Lumen"),
		tostring(offer.port or 34873),
		resolvePlaceName()
	)
	resetButtons()
	widget.Enabled = true
	shownGen = offer.generation or 0
end

connectBtn.MouseButton1Click:Connect(function()
	if not currentOffer then
		return
	end
	jsonPost("/bind", {
		placeName = resolvePlaceName(),
		placeId = game.PlaceId,
		accepted = true,
	})
	title.Text = "Place liée à Lumen"
	body.Text = string.format(
		"Cette place est liée à « %s ».\nPlugins → Rojo → Connect\nHôte 127.0.0.1  ·  port %s\nPas 34872 : c’est VibeStarter, ça casse protocolVersion.",
		tostring(currentOffer.projectName or "Lumen"),
		tostring(currentOffer.port or 34873)
	)
	connectedUi = true
	connectBtn.Visible = false
	laterBtn.Text = "OK"
end)

laterBtn.MouseButton1Click:Connect(function()
	if currentOffer and not connectedUi then
		dismissedGen = currentOffer.generation or dismissedGen
		plugin:SetSetting("lumenDismissedGen", dismissedGen)
	end
	widget.Enabled = false
	resetButtons()
end)

button.Click:Connect(function()
	if widget.Enabled then
		widget.Enabled = false
		return
	end
	local offer = jsonGet("/offer")
	if offer and offer.serving then
		present(offer)
	else
		title.Text = "Lumen"
		body.Text = "Aucun projet en sync. Ouvre un projet dans Lumen — la sync démarre toute seule."
		connectBtn.Visible = false
		laterBtn.Text = "OK"
		widget.Enabled = true
	end
end)

task.spawn(function()
	while true do
		heartbeat()
		local offer = jsonGet("/offer")
		local serving = offer and offer.serving
		button:SetActive(serving == true)
		if offer and offer.wantTextures then
			jsonPost("/textures", { items = scanTextures() })
		end
		if serving then
			local gen = offer.generation or 0
			if gen ~= dismissedGen and gen ~= shownGen then
				present(offer)
			end
		end
		task.wait(2)
	end
end)
"#;
    format!(
        r#"<roblox xmlns:xmime="http://www.w3.org/2005/05/xmlmime" version="4">
  <Item class="Script">
    <Properties>
      <string name="Name">LumenSync</string>
      <ProtectedString name="Source"><![CDATA[{source}]]></ProtectedString>
    </Properties>
  </Item>
</roblox>
"#
    )
}

import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

/** Même format que tools/lumen-vfx.mjs (assets/vfx/<Nom>.vfx.json). */
type Seq = [number, number][];
type ColorSeq = [number, string][];
export type Layer = {
  name: string;
  texture?: string;
  color: ColorSeq;
  size: Seq;
  transparency: Seq;
  lifetime: [number, number];
  speed: [number, number];
  spread?: [number, number];
  acceleration?: [number, number, number];
  drag?: number;
  lightEmission?: number;
  rotation?: [number, number];
  rotSpeed?: [number, number];
  zOffset?: number;
  lockedToPart?: boolean;
  offset?: [number, number, number];
  emits?: [number, number][];
  rate?: number;
  rateFrom?: number;
  rateTo?: number;
};
type Spec = { name: string; duration: number; loop?: boolean; layers: Layer[] };
type VfxFile = { name: string; path: string; modified: number; text: string };

const MAX = 600; // particules par couche dans l'aperçu (Roblox plafonne à 400/s par émetteur)

const rand = (r: [number, number] | undefined, d = 0) => (r ? r[0] + Math.random() * (r[1] - r[0]) : d);

function evalSeq(seq: Seq, t: number) {
  if (!seq.length) return 0;
  if (t <= seq[0][0]) return seq[0][1];
  for (let i = 0; i < seq.length - 1; i++) {
    const [t0, v0] = seq[i];
    const [t1, v1] = seq[i + 1];
    if (t <= t1) return v0 + ((v1 - v0) * (t - t0)) / Math.max(t1 - t0, 1e-6);
  }
  return seq[seq.length - 1][1];
}

function evalColor(seq: ColorSeq, t: number, out: THREE.Color) {
  const c0 = new THREE.Color();
  const c1 = new THREE.Color();
  if (t <= seq[0][0]) return out.set(seq[0][1]);
  for (let i = 0; i < seq.length - 1; i++) {
    if (t <= seq[i + 1][0]) {
      c0.set(seq[i][1]);
      c1.set(seq[i + 1][1]);
      return out.copy(c0).lerp(c1, (t - seq[i][0]) / Math.max(seq[i + 1][0] - seq[i][0], 1e-6));
    }
  }
  return out.set(seq[seq.length - 1][1]);
}

const VERT = `
attribute vec3 aColor; attribute float aAlpha; attribute float aSize; attribute float aRot;
uniform float uScale;
varying vec3 vColor; varying float vAlpha; varying float vRot;
void main() {
  vColor = aColor; vAlpha = aAlpha; vRot = aRot;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = aSize * uScale / max(-mv.z, 0.01);
  gl_Position = projectionMatrix * mv;
}`;
const FRAG = `
uniform sampler2D uTex; uniform float uEmission;
varying vec3 vColor; varying float vAlpha; varying float vRot;
void main() {
  vec2 p = gl_PointCoord - 0.5;
  float c = cos(vRot), s = sin(vRot);
  p = vec2(c * p.x - s * p.y, s * p.x + c * p.y) + 0.5;
  if (p.x < 0.0 || p.x > 1.0 || p.y < 0.0 || p.y > 1.0) discard;
  vec4 t = texture2D(uTex, vec2(p.x, 1.0 - p.y));
  float a = t.a * vAlpha;
  // LightEmission : 0 = mélange normal, 1 = additif (comme Roblox).
  gl_FragColor = vec4(vColor * t.rgb * a, a * (1.0 - uEmission));
}`;

type Particle = { pos: THREE.Vector3; vel: THREE.Vector3; age: number; life: number; rot: number; rotSpeed: number };

export class LayerSim {
  layer: Layer;
  particles: Particle[] = [];
  points: THREE.Points;
  geo = new THREE.BufferGeometry();
  emitted = new Set<number>();
  rateAcc = 0;
  constructor(layer: Layer, texture: THREE.Texture, index: number) {
    this.layer = layer;
    this.geo.setAttribute("position", new THREE.BufferAttribute(new Float32Array(MAX * 3), 3));
    this.geo.setAttribute("aColor", new THREE.BufferAttribute(new Float32Array(MAX * 3), 3));
    this.geo.setAttribute("aAlpha", new THREE.BufferAttribute(new Float32Array(MAX), 1));
    this.geo.setAttribute("aSize", new THREE.BufferAttribute(new Float32Array(MAX), 1));
    this.geo.setAttribute("aRot", new THREE.BufferAttribute(new Float32Array(MAX), 1));
    this.geo.setDrawRange(0, 0);
    const mat = new THREE.ShaderMaterial({
      vertexShader: VERT,
      fragmentShader: FRAG,
      uniforms: { uTex: { value: texture }, uScale: { value: 400 }, uEmission: { value: layer.lightEmission ?? 0 } },
      transparent: true,
      depthWrite: false,
      blending: THREE.CustomBlending,
      blendSrc: THREE.OneFactor,
      blendDst: THREE.OneMinusSrcAlphaFactor,
    });
    this.points = new THREE.Points(this.geo, mat);
    this.points.frustumCulled = false;
    this.points.renderOrder = Math.round((layer.zOffset ?? 0) * 10) + index;
  }
  spawn(count: number) {
    const l = this.layer;
    const o = l.offset ?? [0, 0, 0];
    const [sx, sz] = l.spread ?? [0, 0];
    for (let i = 0; i < count && this.particles.length < MAX; i++) {
      const dir = new THREE.Vector3(0, 1, 0)
        .applyAxisAngle(new THREE.Vector3(1, 0, 0), THREE.MathUtils.degToRad((Math.random() * 2 - 1) * sx))
        .applyAxisAngle(new THREE.Vector3(0, 0, 1), THREE.MathUtils.degToRad((Math.random() * 2 - 1) * sz));
      this.particles.push({
        pos: new THREE.Vector3(o[0], o[1], o[2]),
        vel: dir.multiplyScalar(rand(l.speed)),
        age: 0,
        life: Math.max(rand(l.lifetime, 1), 0.01),
        rot: THREE.MathUtils.degToRad(rand(l.rotation)),
        rotSpeed: THREE.MathUtils.degToRad(rand(l.rotSpeed)),
      });
    }
  }
  reset() {
    this.particles = [];
    this.emitted.clear();
    this.rateAcc = 0;
  }
  step(t: number, dt: number, duration: number) {
    const l = this.layer;
    (l.emits ?? []).forEach(([at, n], i) => {
      if (t >= at && !this.emitted.has(i)) {
        this.emitted.add(i);
        this.spawn(n);
      }
    });
    if ((l.rate ?? 0) > 0 && t >= (l.rateFrom ?? 0) && t < (l.rateTo ?? duration)) {
      this.rateAcc += (l.rate ?? 0) * dt;
      const n = Math.floor(this.rateAcc);
      this.rateAcc -= n;
      this.spawn(n);
    }
    const acc = new THREE.Vector3(...(l.acceleration ?? [0, 0, 0]));
    const decay = Math.pow(0.5, (l.drag ?? 0) * dt); // la vitesse est divisée par 2 toutes les 1/Drag s
    this.particles = this.particles.filter((p) => (p.age += dt) < p.life);
    const pos = this.geo.getAttribute("position") as THREE.BufferAttribute;
    const col = this.geo.getAttribute("aColor") as THREE.BufferAttribute;
    const alpha = this.geo.getAttribute("aAlpha") as THREE.BufferAttribute;
    const size = this.geo.getAttribute("aSize") as THREE.BufferAttribute;
    const rot = this.geo.getAttribute("aRot") as THREE.BufferAttribute;
    const c = new THREE.Color();
    this.particles.forEach((p, i) => {
      p.vel.addScaledVector(acc, dt).multiplyScalar(decay);
      p.pos.addScaledVector(p.vel, dt);
      p.rot += p.rotSpeed * dt;
      const k = p.age / p.life;
      pos.setXYZ(i, p.pos.x, p.pos.y, p.pos.z);
      evalColor(l.color, k, c);
      col.setXYZ(i, c.r, c.g, c.b);
      alpha.setX(i, 1 - THREE.MathUtils.clamp(evalSeq(l.transparency, k), 0, 1));
      size.setX(i, evalSeq(l.size, k));
      rot.setX(i, p.rot);
    });
    for (const a of [pos, col, alpha, size, rot]) a.needsUpdate = true;
    this.geo.setDrawRange(0, this.particles.length);
  }
}

export function checkerTexture(dark: boolean) {
  const cv = document.createElement("canvas");
  cv.width = cv.height = 64;
  const g = cv.getContext("2d")!;
  const [a, b] = dark ? ["#1d2230", "#262c3c"] : ["#d9dee6", "#c6ccd6"];
  for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
    g.fillStyle = (x + y) % 2 ? a : b;
    g.fillRect(x * 8, y * 8, 8, 8);
  }
  const tex = new THREE.CanvasTexture(cv);
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  tex.repeat.set(6, 6); // cases de 5 studs sur 240 studs
  tex.magFilter = THREE.NearestFilter;
  return tex;
}

export function VfxPage({ projectPath }: { projectPath: string }) {
  const [files, setFiles] = useState<VfxFile[]>([]);
  const [selected, setSelected] = useState("");
  const [dark, setDark] = useState(true);
  const [dummy, setDummy] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [loop, setLoop] = useState(true);
  const [replay, setReplay] = useState(0);
  const [time, setTime] = useState(0);
  const hostRef = useRef<HTMLDivElement>(null);
  const world = useRef<{
    renderer: THREE.WebGLRenderer;
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    controls: OrbitControls;
    floor: THREE.Mesh;
    dummy: THREE.Group;
    sims: LayerSim[];
  } | null>(null);
  const settings = useRef({ speed, loop });
  settings.current = { speed, loop };

  // Liste des effets, relue toutes les 1,5 s : l'aperçu suit les modifications de l'agent.
  useEffect(() => {
    let alive = true;
    const load = () =>
      invoke<VfxFile[]>("list_vfx", { projectPath })
        .then((list) => {
          if (!alive) return;
          setFiles((prev) =>
            prev.length === list.length && prev.every((f, i) => f.modified === list[i].modified && f.name === list[i].name)
              ? prev
              : list,
          );
        })
        .catch(() => {});
    void load();
    const timer = window.setInterval(load, 1500);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [projectPath]);

  useEffect(() => {
    if (!selected && files.length) setSelected(files[0].name);
  }, [files, selected]);

  const current = files.find((f) => f.name === selected);
  const parsed = useMemo(() => {
    if (!current) return { spec: null as Spec | null, error: "" };
    try {
      const spec = JSON.parse(current.text) as Spec;
      if (!Array.isArray(spec.layers)) throw new Error("« layers » manquant");
      return { spec, error: "" };
    } catch (err) {
      return { spec: null, error: String(err) };
    }
  }, [current]);

  // Scène three.js, créée une fois.
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.LinearSRGBColorSpace; // couleurs du JSON affichées telles quelles
    host.appendChild(renderer.domElement);
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 1000);
    camera.position.set(14, 9, 18);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, 2.5, 0);
    controls.enableDamping = true;
    const floor = new THREE.Mesh(new THREE.PlaneGeometry(240, 240), new THREE.MeshBasicMaterial({ map: checkerTexture(true) }));
    floor.rotation.x = -Math.PI / 2;
    scene.add(floor);
    const dummyGroup = new THREE.Group();
    const grey = new THREE.MeshBasicMaterial({ color: 0x8c93a1 });
    const part = (w: number, h: number, d: number, x: number, y: number) => {
      const m = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), grey);
      m.position.set(x, y, 0);
      dummyGroup.add(m);
    };
    part(2, 2, 1, 0, 3);
    part(1.2, 1.2, 1.2, 0, 4.6);
    part(1, 2, 1, -0.5, 1);
    part(1, 2, 1, 0.5, 1);
    part(1, 2, 1, -1.5, 3);
    part(1, 2, 1, 1.5, 3);
    dummyGroup.position.set(-5, 0, 0);
    scene.add(dummyGroup);
    world.current = { renderer, scene, camera, controls, floor, dummy: dummyGroup, sims: [] };

    const resize = () => {
      const w = host.clientWidth;
      const h = host.clientHeight;
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      const scale = (h * renderer.getPixelRatio()) / (2 * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)));
      world.current?.sims.forEach((s) => ((s.points.material as THREE.ShaderMaterial).uniforms.uScale.value = scale));
    };
    const ro = new ResizeObserver(resize);
    ro.observe(host);
    resize();
    return () => {
      ro.disconnect();
      controls.dispose();
      renderer.dispose();
      host.removeChild(renderer.domElement);
      world.current = null;
    };
  }, []);

  useEffect(() => {
    const w = world.current;
    if (!w) return;
    (w.floor.material as THREE.MeshBasicMaterial).map = checkerTexture(dark);
    (w.floor.material as THREE.MeshBasicMaterial).needsUpdate = true;
    w.scene.background = new THREE.Color(dark ? 0x10131c : 0xeef1f6);
  }, [dark]);

  useEffect(() => {
    if (world.current) world.current.dummy.visible = dummy;
  }, [dummy]);

  // (Re)construit les couches et joue l'effet.
  useEffect(() => {
    const w = world.current;
    const spec = parsed.spec;
    if (!w || !spec) return;
    const loader = new THREE.TextureLoader();
    const sims = spec.layers.map((layer, i) => {
      const file = !layer.texture
        ? "assets/vfx/textures/glow.png"
        : /\.(png|jpe?g)$/i.test(layer.texture)
          ? layer.texture
          : `assets/vfx/textures/${layer.texture}.png`;
      const tex = loader.load(convertFileSrc(`${projectPath}/${file}`.replace(/\\/g, "/")));
      tex.colorSpace = THREE.NoColorSpace;
      const sim = new LayerSim(layer, tex, i);
      w.scene.add(sim.points);
      return sim;
    });
    w.sims = sims;
    const host = hostRef.current;
    if (host) {
      const scale = (host.clientHeight * w.renderer.getPixelRatio()) / (2 * Math.tan(THREE.MathUtils.degToRad(w.camera.fov / 2)));
      sims.forEach((s) => ((s.points.material as THREE.ShaderMaterial).uniforms.uScale.value = scale));
    }
    const tail = Math.max(0, ...spec.layers.map((l) => l.lifetime?.[1] ?? 0));
    let t = 0;
    let last = performance.now();
    let frame = 0;
    let shown = -1;
    const tick = () => {
      const now = performance.now();
      const dt = Math.min((now - last) / 1000, 0.05) * settings.current.speed;
      last = now;
      t += dt;
      if (t > spec.duration + (settings.current.loop ? 0 : tail)) {
        if (settings.current.loop) {
          t = 0;
          sims.forEach((s) => s.reset());
        } else t = spec.duration + tail;
      }
      sims.forEach((s) => s.step(t, dt, spec.duration));
      w.controls.update();
      w.renderer.render(w.scene, w.camera);
      const rounded = Math.round(t * 10) / 10;
      if (rounded !== shown) {
        shown = rounded;
        setTime(rounded);
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      sims.forEach((s) => {
        w.scene.remove(s.points);
        s.geo.dispose();
        (s.points.material as THREE.ShaderMaterial).dispose();
      });
      w.sims = [];
    };
  }, [parsed, projectPath, replay]);

  return (
    <div className="vfx-page">
      <h1>VFX</h1>
      <p className="lede">
        Aperçu des effets du projet (<code>assets/vfx/*.vfx.json</code>). Il se met à jour quand l’agent modifie un
        fichier. C’est une approximation du moteur Roblox : valide toujours le rendu final dans Studio.
      </p>
      <div className="vfx-layout">
        <aside className="vfx-list">
          {files.length === 0 ? <p className="muted">Aucun effet dans assets/vfx/.</p> : null}
          {files.map((f) => (
            <button
              key={f.name}
              type="button"
              className={f.name === selected ? "on" : ""}
              onClick={() => {
                setSelected(f.name);
                setReplay((r) => r + 1);
              }}
            >
              {f.name}
            </button>
          ))}
        </aside>
        <div className="vfx-stage-wrap">
          <div className="vfx-stage" ref={hostRef} />
          {parsed.error ? <p className="vfx-error">JSON invalide : {parsed.error}</p> : null}
          <div className="vfx-controls">
            <button type="button" className="btn secondary" onClick={() => setReplay((r) => r + 1)}>
              Rejouer
            </button>
            <button type="button" className={`btn secondary ${loop ? "on" : ""}`} onClick={() => setLoop((v) => !v)}>
              {loop ? "Boucle : oui" : "Boucle : non"}
            </button>
            <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="Vitesse">
              <option value={1}>Vitesse 1×</option>
              <option value={0.5}>Ralenti 0,5×</option>
              <option value={0.25}>Ralenti 0,25×</option>
              <option value={0.1}>Ralenti 0,1×</option>
            </select>
            <button type="button" className="btn secondary" onClick={() => setDark((v) => !v)}>
              {dark ? "Sol sombre" : "Sol clair"}
            </button>
            <button type="button" className="btn secondary" onClick={() => setDummy((v) => !v)}>
              {dummy ? "Mannequin 5 studs : oui" : "Mannequin : non"}
            </button>
            <span className="vfx-time">
              {time.toFixed(1)} s{parsed.spec ? ` / ${parsed.spec.duration} s · ${parsed.spec.layers.length} couches` : ""}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

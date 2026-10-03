// IFCX demo viewer (#47).
//
// Everything runs in the page: @openbim/ifcx (the `web` build, from this
// repository's source) validates and composes the layers and exports the
// composed model as GLB; three.js loads that GLB. The GLB's root node turns
// IFCX's Z-up into glTF's Y-up, and each instance node is named by its IFCX
// path and carries the IFCX node id in `extras.ifcxNode` (three.js:
// `userData.ifcxNode`), which is how a picked mesh finds its place in the
// composed tree.
import init, { IfcxFile, compose, exportGlb, fetchImports, validate } from "@openbim/ifcx/web";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";

// The repository's own hand-written fixtures (MIT, see their README). No
// buildingSMART file is hosted or fetched.
import geometryModel from "../../crates/openbim-ifcx/tests/fixtures/geometry-model.ifcx?url";
import invalidAttributes from "../../crates/openbim-ifcx/tests/fixtures/invalid-attributes.ifcx?url";
import occurrenceType from "../../crates/openbim-ifcx/tests/fixtures/occurrence-type.ifcx?url";

const SAMPLES = [
  { label: "Pavilion: columns, axes and a roof (geometry-model.ifcx)", url: geometryModel, name: "geometry-model.ifcx" },
  { label: "Occurrences of a type, no renderable geometry (occurrence-type.ifcx)", url: occurrenceType, name: "occurrence-type.ifcx" },
  { label: "Attributes that fail validation (invalid-attributes.ifcx)", url: invalidAttributes, name: "invalid-attributes.ifcx" },
];

const $ = (id) => document.getElementById(id);
const statusEl = $("status");
const treeEl = $("tree");
const selectionEl = $("selection");
const validationEl = $("validation");
const viewport = $("viewport");
const sampleEl = $("sample");
const fetchToggle = $("fetch-imports");

function setStatus(text, error = false) {
  statusEl.textContent = text;
  statusEl.classList.toggle("error", error);
}

function el(tag, props = {}, ...children) {
  const node = Object.assign(document.createElement(tag), props);
  node.append(...children.filter((c) => c !== undefined && c !== null));
  return node;
}

// ---------------------------------------------------------------- viewer

const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
renderer.outputColorSpace = THREE.SRGBColorSpace;
viewport.prepend(renderer.domElement);

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000);
camera.position.set(8, 6, 8);
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;

scene.add(new THREE.HemisphereLight(0xffffff, 0x8d939e, 1.6));
const sun = new THREE.DirectionalLight(0xffffff, 2.2);
const fill = new THREE.DirectionalLight(0xffffff, 0.6);
scene.add(sun, sun.target, fill, fill.target);
let grid;
let model;

function resize() {
  const { clientWidth: w, clientHeight: h } = viewport;
  if (w === 0 || h === 0) return;
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}
new ResizeObserver(resize).observe(viewport);
resize();
renderer.setAnimationLoop(() => {
  controls.update();
  renderer.render(scene, camera);
});

function disposeModel() {
  if (!model) return;
  scene.remove(model);
  model.traverse((object) => {
    object.geometry?.dispose();
    for (const material of [object.material, object.userData.original].flat()) material?.dispose?.();
  });
  model = undefined;
  if (grid) {
    scene.remove(grid);
    grid.dispose();
    grid = undefined;
  }
}

/** Frame the model: camera, controls, lights and a grid fitted to its bounds. */
function fitToBounds(object) {
  const box = new THREE.Box3().setFromObject(object);
  if (box.isEmpty()) return false;
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  const radius = Math.max(size.length() / 2, 1e-3);
  const distance = (1.1 * radius) / Math.sin(THREE.MathUtils.degToRad(camera.fov) / 2);
  const direction = new THREE.Vector3(1, 0.7, 1.2).normalize();
  camera.position.copy(center).addScaledVector(direction, distance);
  camera.near = distance / 1000;
  camera.far = distance * 100;
  camera.updateProjectionMatrix();
  controls.target.copy(center);
  controls.update();
  sun.position.copy(center).add(new THREE.Vector3(radius, radius * 2, radius * 1.5));
  sun.target.position.copy(center);
  fill.position.copy(center).add(new THREE.Vector3(-radius * 1.5, radius, -radius));
  fill.target.position.copy(center);
  const extent = Math.max(size.x, size.z) * 2 || 1;
  const step = 10 ** Math.floor(Math.log10(extent / 2));
  grid = new THREE.GridHelper(Math.ceil(extent / step) * step, Math.ceil(extent / step), 0x8d939e, 0xc3c8d0);
  grid.material.transparent = true;
  grid.material.opacity = 0.5;
  grid.position.set(center.x, box.min.y, center.z);
  scene.add(grid);
  return true;
}

// ------------------------------------------------------------- selection

const HIGHLIGHT = new THREE.Color(0x1f6feb);
let tree; // the composed tree
let selected; // IFCX path of the selection
let treeButtons = new Map(); // IFCX path -> tree button, for rendered rows
let meshPaths = new Set(); // IFCX paths of GLB instances

function highlight(object, on) {
  if (!object.material) return;
  if (on) {
    object.userData.original = object.material;
    const material = object.material.clone();
    if (material.emissive) {
      material.emissive.copy(HIGHLIGHT);
      material.emissiveIntensity = 0.55;
    } else if (material.color) {
      material.color.copy(HIGHLIGHT);
    }
    object.material = material;
  } else if (object.userData.original) {
    object.material.dispose();
    object.material = object.userData.original;
    delete object.userData.original;
  }
}

/**
 * The IFCX path of a GLB instance object. GLTFLoader sanitises `name` for
 * animation bindings ("/" becomes "_") and keeps the original in
 * `userData.name`.
 */
const ifcxPath = (object) => object.userData.name ?? object.name;

/** The GLB instance objects at or below an IFCX path. */
function instancesAt(path) {
  const found = [];
  model?.traverse((object) => {
    if (object.userData.ifcxNode === undefined) return;
    const at = ifcxPath(object);
    if (at === path || at.startsWith(`${path}/`)) found.push(object);
  });
  return found;
}

/** The composed node at an IFCX path, by child names from the root. */
function nodeAt(path) {
  let node = tree;
  for (const name of path.split("/")) {
    node = node?.children[name];
  }
  return node;
}

/**
 * JSON for the panel: arrays of plain values on one line, and long arrays
 * cut short, so geometry does not drown the attributes.
 */
function summarise(value, indent = "") {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  const inner = `${indent}  `;
  if (Array.isArray(value)) {
    const shown = value.slice(0, 6).map((v) => summarise(v, inner));
    if (value.length > 6) shown.push(`… ${value.length - 6} more`);
    const flat = value.every((v) => v === null || typeof v !== "object");
    return flat ? `[${shown.join(", ")}]` : `[\n${shown.map((v) => inner + v).join(",\n")}\n${indent}]`;
  }
  const entries = Object.entries(value);
  if (entries.length === 0) return "{}";
  return `{\n${entries.map(([k, v]) => `${inner}${JSON.stringify(k)}: ${summarise(v, inner)}`).join(",\n")}\n${indent}}`;
}

function select(path) {
  if (selected !== undefined) {
    for (const object of instancesAt(selected)) highlight(object, false);
    treeButtons.get(selected)?.classList.remove("selected");
  }
  selected = path;
  if (path === undefined) {
    selectionEl.replaceChildren(el("p", { className: "hint", textContent: "Click a mesh or a node." }));
    return;
  }
  const instances = instancesAt(path);
  for (const object of instances) highlight(object, true);
  revealInTree(path);

  const node = nodeAt(path);
  const own = instances.find((o) => ifcxPath(o) === path);
  const rows = [
    ["IFCX path", path],
    ["Node", node?.path ?? own?.userData.ifcxNode ?? "?"],
    ["Meshes", String(instances.length)],
  ];
  const attributes = Object.entries(node?.attributes ?? {});
  selectionEl.replaceChildren(
    el("dl", {}, ...rows.flatMap(([k, v]) => [el("dt", { textContent: k }), el("dd", { textContent: v })])),
    attributes.length === 0
      ? el("p", { className: "hint", textContent: "No attributes." })
      : el(
          "div",
          {},
          ...attributes.map(([id, value]) =>
            el("details", { open: attributes.length <= 6 }, el("summary", { textContent: id }), el("pre", { textContent: summarise(value) })),
          ),
        ),
  );
}

// ------------------------------------------------------------- tree panel

function treeRow(name, node, path) {
  const childNames = Object.keys(node.children);
  // A leaf is a button; a branch is its <details>' summary, so a click
  // both selects the node and opens or closes it.
  const row = el(
    childNames.length === 0 ? "button" : "summary",
    { className: "row", title: `${path}\nnode ${node.path}` },
    name,
    meshPaths.has(path) ? el("span", { className: "mesh", textContent: " ■" }) : undefined,
  );
  if (childNames.length === 0) row.type = "button";
  row.addEventListener("click", () => select(path));
  treeButtons.set(path, row);
  if (path === selected) row.classList.add("selected");
  if (childNames.length === 0) return el("li", {}, row);
  // Children render when first opened: real models have many nodes.
  const list = el("ul");
  const details = el("details", {}, row, list);
  const populate = () => {
    if (details.open && list.childElementCount === 0) {
      list.append(...childNames.map((child) => treeRow(child, node.children[child], `${path}/${child}`)));
    }
  };
  details.addEventListener("toggle", populate);
  details.populate = populate;
  return el("li", {}, details);
}

function renderTree() {
  treeButtons = new Map();
  const roots = Object.keys(tree.children);
  if (roots.length === 0) {
    treeEl.replaceChildren(el("p", { className: "hint", textContent: "No nodes." }));
    return;
  }
  treeEl.replaceChildren(el("ul", {}, ...roots.map((name) => treeRow(name, tree.children[name], name))));
  // Open the first levels so the structure is visible.
  for (const details of treeEl.querySelectorAll(":scope > ul > li > details")) {
    details.open = true;
    details.populate();
  }
}

/** Open every ancestor of `path` in the tree and scroll its row into view. */
function revealInTree(path) {
  const names = path.split("/");
  for (let i = 1; i < names.length; i++) {
    const details = treeButtons.get(names.slice(0, i).join("/"))?.closest("details");
    if (details) {
      details.open = true;
      details.populate();
    }
  }
  const button = treeButtons.get(path);
  button?.classList.add("selected");
  button?.scrollIntoView({ block: "nearest" });
}

// ------------------------------------------------------------- validation

function renderValidation(report, notes) {
  const items = notes.map((text) => el("p", { className: "failure", textContent: text }));
  if (report === undefined) {
    validationEl.replaceChildren(...items);
    return;
  }
  if (report.valid) {
    items.unshift(el("p", { className: "valid", textContent: "Every attribute matches its schema." }));
  } else {
    items.unshift(el("p", { textContent: `${report.failures.length} attribute value(s) fail validation:` }));
    for (const f of report.failures) {
      items.push(
        el(
          "div",
          { className: "failure" },
          el("div", {}, el("code", { textContent: `${f.node} · ${f.attribute}${f.pointer}` })),
          el("div", { textContent: `${f.kind}: ${f.message}` }),
        ),
      );
    }
  }
  validationEl.replaceChildren(...items);
}

// ----------------------------------------------------------------- loading

const loader = new GLTFLoader();

/** Import URIs of one file and, transitively, of the files at hand. */
function unresolvedImports(inputs, local) {
  const missing = new Set();
  const seen = new Set();
  const queue = [...inputs];
  while (queue.length > 0) {
    let imports = [];
    try {
      imports = IfcxFile.parse(queue.shift()).toJSON().imports ?? [];
    } catch {
      continue; // compose reports the read error
    }
    for (const { uri } of imports) {
      if (seen.has(uri)) continue;
      seen.add(uri);
      if (local.has(uri)) queue.push(local.get(uri));
      else missing.add(uri);
    }
  }
  return { any: seen.size > 0, missing: [...missing] };
}

/**
 * Validate, compose and show layers, weakest first. `files` are
 * `{ name, bytes, url? }`; `url` is what relative imports resolve against.
 */
async function show(files) {
  const started = performance.now();
  setStatus(`Composing ${files.map((f) => f.name).join(" + ")}…`);
  select(undefined);
  disposeModel();
  tree = undefined;
  meshPaths = new Set();
  treeEl.replaceChildren();
  const inputs = files.map((f) => f.bytes);
  // Files opened together also serve each other's imports, by file name.
  const local = new Map(files.map((f) => [f.name, f.bytes]));
  const notes = [];
  let report;
  try {
    let imports;
    if (fetchToggle.checked) {
      imports = await fetchImports(inputs, { baseUrl: files[0].url ?? location.href, imports: local });
    } else {
      const { any, missing } = unresolvedImports(inputs, local);
      if (any && missing.length === 0) imports = local;
      if (missing.length > 0) {
        notes.push(`Imports not resolved (turn on “fetch imports” or open them together): ${missing.join(", ")}`);
      }
    }
    const options = imports ? { imports } : {};
    report = validate(inputs, options);
    tree = compose(inputs, options);
    const glb = exportGlb(inputs, { ...options, originOnRoot: false });
    const gltf = await loader.parseAsync(glb.buffer.slice(glb.byteOffset, glb.byteOffset + glb.byteLength), "");
    model = gltf.scene;
    model.traverse((object) => {
      if (object.userData.ifcxNode !== undefined) meshPaths.add(ifcxPath(object));
    });
    scene.add(model);
    renderTree();
    renderValidation(report, notes);
    const framed = fitToBounds(model);
    const ms = Math.round(performance.now() - started);
    setStatus(
      `${files.map((f) => f.name).join(" + ")}: ${meshPaths.size} instance(s), ${(glb.length / 1024).toFixed(1)} KiB GLB, ${ms} ms` +
        (framed ? "" : ". No renderable geometry; the tree still shows the composed nodes."),
    );
  } catch (error) {
    const code = error?.code ? ` [${error.code}]` : "";
    setStatus(`Failed${code}: ${error?.message ?? error}`, true);
    if (tree) renderTree();
    renderValidation(report, notes);
  }
}

async function readFiles(fileList) {
  const files = await Promise.all(
    [...fileList].map(async (file) => ({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) })),
  );
  if (files.length > 0) {
    sampleEl.value = "";
    await show(files);
  }
}

async function loadSample(index) {
  const sample = SAMPLES[index];
  const url = new URL(sample.url, location.href);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${sample.name}: HTTP ${response.status}`);
  await show([{ name: sample.name, bytes: new Uint8Array(await response.arrayBuffer()), url: url.href }]);
}

// ----------------------------------------------------------------- events

const pointer = new THREE.Vector2();
const raycaster = new THREE.Raycaster();
let down;
renderer.domElement.addEventListener("pointerdown", (event) => {
  down = [event.clientX, event.clientY];
});
renderer.domElement.addEventListener("pointerup", (event) => {
  if (!down || !model || Math.hypot(event.clientX - down[0], event.clientY - down[1]) > 4) return;
  const rect = renderer.domElement.getBoundingClientRect();
  pointer.set(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1);
  raycaster.setFromCamera(pointer, camera);
  raycaster.params.Line.threshold = camera.position.distanceTo(controls.target) / 200;
  raycaster.params.Points.threshold = raycaster.params.Line.threshold;
  const hits = raycaster.intersectObject(model, true);
  // Prefer surfaces over the lines and points drawn on them.
  const hit = hits.find((h) => h.object.isMesh) ?? hits[0];
  let object = hit?.object;
  while (object && object.userData.ifcxNode === undefined) object = object.parent;
  select(object ? ifcxPath(object) : undefined);
});

$("file").addEventListener("change", (event) => readFiles(event.target.files));
for (const target of [viewport, document.body]) {
  target.addEventListener("dragover", (event) => {
    event.preventDefault();
    viewport.classList.add("dragging");
  });
  target.addEventListener("dragleave", () => viewport.classList.remove("dragging"));
  target.addEventListener("drop", (event) => {
    event.preventDefault();
    event.stopPropagation();
    viewport.classList.remove("dragging");
    readFiles(event.dataTransfer.files);
  });
}

SAMPLES.forEach((sample, index) => sampleEl.append(el("option", { value: String(index), textContent: sample.label })));
sampleEl.addEventListener("change", () => {
  if (sampleEl.value !== "") loadSample(Number(sampleEl.value)).catch((e) => setStatus(String(e), true));
});

try {
  await init();
  sampleEl.value = "0";
  await loadSample(0);
} catch (error) {
  setStatus(`Could not start: ${error?.message ?? error}`, true);
}

import {
  AmbientLight, Box3, Color, DirectionalLight, Mesh, MeshStandardMaterial,
  PerspectiveCamera, Scene, Vector3, WebGLRenderer,
} from "three";
import { STLLoader } from "three/examples/jsm/loaders/STLLoader.js";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { fitCamera } from "./stlFit";

export interface BuiltScene {
  readonly triangles: number;
  readonly dispose: () => void;
}

const FALLBACK_COLOR = "#8fa3bd";

/** The accent colour from the theme, or a calm blue-grey if the browser's value is not parseable. */
function modelColor(el: HTMLElement): Color {
  const css = getComputedStyle(el).getPropertyValue("--color-accent").trim();
  const color = new Color(FALLBACK_COLOR);
  if (!css) return color;
  try { color.set(css); } catch { /* keep the fallback */ }
  return color;
}

/** Parse the STL bytes and draw them into `host`. Throws if WebGL or the file is unusable. */
export function buildScene(host: HTMLElement, bytes: ArrayBuffer): BuiltScene {
  const geometry = new STLLoader().parse(bytes);
  const position = geometry.getAttribute("position");
  if (!position || position.count === 0) {
    geometry.dispose();
    throw new Error("empty model");
  }
  // Many exporters write zero normals, so always derive them from the triangles.
  geometry.computeVertexNormals();
  geometry.computeBoundingBox();
  const box = geometry.boundingBox ?? new Box3();
  const center = box.getCenter(new Vector3());
  const size = box.getSize(new Vector3());
  geometry.translate(-center.x, -center.y, -center.z);

  let renderer: WebGLRenderer;
  try {
    renderer = new WebGLRenderer({ antialias: true, alpha: true });
  } catch (error) {
    geometry.dispose();
    throw error;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x000000, 0);
  host.appendChild(renderer.domElement);

  const material = new MeshStandardMaterial({ color: modelColor(host), roughness: 0.55, metalness: 0.1 });
  const scene = new Scene();
  scene.add(new Mesh(geometry, material));
  scene.add(new AmbientLight(0xffffff, 0.45));
  const key = new DirectionalLight(0xffffff, 2.2);
  key.position.set(1, 1.4, 1.2);
  scene.add(key);
  const fill = new DirectionalLight(0xffffff, 0.9);
  fill.position.set(-1.2, -0.4, -1);
  scene.add(fill);

  const camera = new PerspectiveCamera(40, 1, 0.1, 1000);
  const origin = new Vector3();
  fitCamera(camera, origin, size);

  const draw = () => renderer.render(scene, camera);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = false;
  controls.addEventListener("change", draw);

  const resize = () => {
    const width = Math.max(host.clientWidth, 1);
    const height = Math.max(host.clientHeight, 1);
    renderer.setSize(width, height, false);
    renderer.domElement.style.width = "100%";
    renderer.domElement.style.height = "100%";
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    draw();
  };
  const observer = new ResizeObserver(resize);
  observer.observe(host);
  resize();

  return {
    triangles: Math.floor(position.count / 3),
    dispose: () => {
      observer.disconnect();
      controls.removeEventListener("change", draw);
      controls.dispose();
      geometry.dispose();
      material.dispose();
      renderer.dispose();
      renderer.forceContextLoss();
      renderer.domElement.remove();
    },
  };
}

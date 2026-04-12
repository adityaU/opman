/**
 * CadViewer — 3D model viewer using Three.js (loaded via CDN).
 * Supports STL, OBJ, GLTF/GLB, PLY, 3MF, FBX, DAE formats.
 * Matches Leptos cad_viewer.rs + model3d_preview.rs.
 */
import { useEffect, useRef, useId } from "react";
import { rawFileUrl } from "../../api";

interface Props {
  path: string;
}

declare global {
  interface Window {
    __THREE: any;
    __THREE_OrbitControls: any;
    __THREE_Loaders: any;
  }
}

/** Active viewer instances for cleanup. */
const viewers = new Map<string, {
  scene: any; camera: any; renderer: any;
  controls: any; resizeObs: ResizeObserver; animId: number;
}>();

function initViewer(containerId: string, url: string, ext: string) {
  const THREE = window.__THREE;
  const OrbitControls = window.__THREE_OrbitControls;
  const Loaders = window.__THREE_Loaders;
  if (!THREE || !OrbitControls || !Loaders) {
    console.error("[CAD] Three.js not loaded");
    return;
  }

  const container = document.getElementById(containerId);
  if (!container) return;

  // Clean up previous viewer if any
  disposeViewer(containerId);

  const w = container.clientWidth || 600;
  const h = container.clientHeight || 400;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x1a1a2e);

  scene.add(new THREE.GridHelper(20, 20, 0x444466, 0x333355));
  scene.add(new THREE.AmbientLight(0xffffff, 0.6));
  const d1 = new THREE.DirectionalLight(0xffffff, 0.8);
  d1.position.set(5, 10, 7);
  scene.add(d1);
  const d2 = new THREE.DirectionalLight(0xffffff, 0.3);
  d2.position.set(-5, -3, -5);
  scene.add(d2);

  const camera = new THREE.PerspectiveCamera(50, w / h, 0.01, 1000);
  camera.position.set(3, 3, 3);

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
  renderer.setSize(w, h);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  container.appendChild(renderer.domElement);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.1;
  controls.enablePan = true;
  controls.enableZoom = true;

  const resizeObs = new ResizeObserver(() => {
    const cw = container.clientWidth;
    const ch = container.clientHeight;
    if (cw > 0 && ch > 0) {
      camera.aspect = cw / ch;
      camera.updateProjectionMatrix();
      renderer.setSize(cw, ch);
    }
  });
  resizeObs.observe(container);

  let animId = 0;
  function animate() {
    animId = requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }
  animate();

  viewers.set(containerId, { scene, camera, renderer, controls, resizeObs, animId });

  const loadingEl = container.querySelector<HTMLElement>(".cad-loading");
  const e = ext.toLowerCase();

  type LoaderCtor = new () => { load: (url: string, ok: (r: any) => void, progress?: any, err?: (e: any) => void) => void };
  let LoaderClass: LoaderCtor | null = null;
  if (e === "stl") LoaderClass = Loaders.STLLoader;
  else if (e === "obj") LoaderClass = Loaders.OBJLoader;
  else if (e === "gltf" || e === "glb") LoaderClass = Loaders.GLTFLoader;
  else if (e === "ply") LoaderClass = Loaders.PLYLoader;
  else if (e === "3mf") LoaderClass = Loaders.ThreeMFLoader;
  else if (e === "fbx") LoaderClass = Loaders.FBXLoader;
  else if (e === "dae") LoaderClass = Loaders.ColladaLoader;

  if (!LoaderClass) {
    if (loadingEl) loadingEl.textContent = `Unsupported format: ${ext}`;
    return;
  }

  const loader = new LoaderClass();
  loader.load(url, (result: any) => {
    let object: any;
    if (e === "stl" || e === "ply") {
      const geom = result;
      geom.computeVertexNormals();
      object = new THREE.Mesh(geom, new THREE.MeshStandardMaterial({
        color: 0x7799cc, metalness: 0.3, roughness: 0.6, flatShading: false,
      }));
    } else if (e === "gltf" || e === "glb" || e === "dae") {
      object = result.scene;
    } else {
      object = result;
    }

    // Center and scale to fit
    const box = new THREE.Box3().setFromObject(object);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z);
    if (maxDim > 0) {
      object.scale.multiplyScalar(4 / maxDim);
      box.setFromObject(object);
      box.getCenter(center);
    }
    object.position.sub(center);
    scene.add(object);

    const fitBox = new THREE.Box3().setFromObject(object);
    const fitSize = fitBox.getSize(new THREE.Vector3());
    const fitMax = Math.max(fitSize.x, fitSize.y, fitSize.z);
    const dist = fitMax / (2 * Math.tan((camera.fov * Math.PI) / 360));
    camera.position.set(dist * 0.8, dist * 0.6, dist * 0.8);
    controls.target.set(0, fitSize.y * 0.2, 0);
    controls.update();

    if (loadingEl) loadingEl.style.display = "none";
  }, undefined, (err: any) => {
    console.error("[CAD] Load error:", err);
    if (loadingEl) loadingEl.textContent = "Failed to load model";
  });
}

function disposeViewer(containerId: string) {
  const v = viewers.get(containerId);
  if (!v) return;
  cancelAnimationFrame(v.animId);
  v.resizeObs.disconnect();
  v.controls.dispose();
  v.renderer.dispose();
  v.renderer.domElement?.parentNode?.removeChild(v.renderer.domElement);
  v.scene.traverse((obj: any) => {
    obj.geometry?.dispose();
    if (obj.material) {
      if (Array.isArray(obj.material)) obj.material.forEach((m: any) => m.dispose());
      else obj.material.dispose();
    }
  });
  viewers.delete(containerId);
}

export function CadViewer({ path }: Props) {
  const stableId = useId();
  const containerId = `cad-viewer-${stableId.replace(/:/g, "")}`;
  const url = rawFileUrl(path);
  const ext = path.split(".").pop()?.toLowerCase() ?? "stl";
  const name = path.split("/").pop() ?? path;

  const initRef = useRef(false);

  useEffect(() => {
    // Delay slightly to ensure DOM element exists
    const timer = setTimeout(() => {
      initViewer(containerId, url, ext);
      initRef.current = true;
    }, 50);
    return () => {
      clearTimeout(timer);
      if (initRef.current) disposeViewer(containerId);
    };
  }, [containerId, url, ext]);

  return (
    <div className="file-preview file-preview-cad">
      <div className="cad-header">
        <span className="cad-file-name">{name}</span>
        <span className="cad-format-badge">{ext.toUpperCase()}</span>
        <span className="cad-hint">Scroll to zoom &bull; Drag to rotate &bull; Right-click to pan</span>
      </div>
      <div className="cad-canvas-container" id={containerId}>
        <div className="cad-loading">Loading 3D model&hellip;</div>
      </div>
    </div>
  );
}

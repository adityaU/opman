/**
 * Three.js CDN loader — sets window globals for CadViewer.
 * Lives in public/ so Vite copies it as-is (no Rollup processing).
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { STLLoader } from 'three/addons/loaders/STLLoader.js';
import { OBJLoader } from 'three/addons/loaders/OBJLoader.js';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { PLYLoader } from 'three/addons/loaders/PLYLoader.js';
import { ThreeMFLoader } from 'three/addons/loaders/3MFLoader.js';
import { FBXLoader } from 'three/addons/loaders/FBXLoader.js';
import { ColladaLoader } from 'three/addons/loaders/ColladaLoader.js';

window.__THREE = THREE;
window.__THREE_OrbitControls = OrbitControls;
window.__THREE_Loaders = {
  STLLoader, OBJLoader, GLTFLoader, PLYLoader,
  ThreeMFLoader, FBXLoader, ColladaLoader
};

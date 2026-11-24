---
name: web3d
description: Patterns for adding real-time 3D to the melodias-studio site hero or video overlays with Three.js, GSAP ScrollTrigger and React Three Fiber. Use when a task asks for interactive 3D on the page, scroll-driven camera work, WebGL particles behind content, or animated 3D dressing in the Remotion video. Adapted from the web3d-integration-patterns skill by freshtechbro.
---

# Web3D for melodias-studio

Three.js covers the imperative scenes, GSAP ScrollTrigger drives scroll-linked motion,
and React Three Fiber fits the Remotion video project in video/, which is already React.
Plain CSS and the Mercury system in site/DESIGN.md stay the default; reach for WebGL only
when a static image or video cannot carry the moment.

## Where 3D may live

- Site hero in site/index.html with behaviour in site/assets/js/main.js: a lightweight
  WebGL canvas behind the headline, pointer parallax, no scroll hijacking.
- Video overlays: the Remotion project under video/ renders the waveform and particles
  today. A Three.js or React Three Fiber layer can add depth there, but it must render
  headless under `make video` and `make render` with no browser window.
- Performance budget: the page stays under about 12 MB and must hold 60 fps on
  integrated graphics. Prefer one canvas, capped pixel ratio, demand rendering.

## Pattern 1: hero canvas with pointer parallax

Use for the landing hero. One fixed scene, no router, no framework.

```javascript
// site/assets/js/hero-3d.js
import * as THREE from 'three';

export function initHero(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
  camera.position.set(0, 0, 8);

  const group = new THREE.Group();
  scene.add(group);
  // add low-poly geometry here, keep draw calls under a dozen

  const target = { x: 0, y: 0 };
  window.addEventListener('pointermove', (e) => {
    target.x = (e.clientX / window.innerWidth - 0.5) * 0.6;
    target.y = (e.clientY / window.innerHeight - 0.5) * 0.4;
  });

  function resize() {
    const w = canvas.clientWidth || window.innerWidth;
    const h = canvas.clientHeight || 480;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  window.addEventListener('resize', resize);
  resize();

  renderer.setAnimationLoop(() => {
    group.rotation.y += (target.x - group.rotation.y) * 0.05;
    group.rotation.x += (target.y - group.rotation.x) * 0.05;
    renderer.render(scene, camera);
  });

  return () => renderer.dispose();
}
```

Rules: alpha canvas over the onyx background, never a second scroll container,
pause the loop when the hero leaves the viewport with IntersectionObserver,
honour prefers-reduced-motion by rendering one static frame.

## Pattern 2: scroll-driven camera with GSAP

Use when the page needs a camera move tied to reading progress, for example a
slow orbit while the pipeline section scrolls by.

```javascript
// scroll rig, loaded after the main bundle
import gsap from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';

gsap.registerPlugin(ScrollTrigger);

export function initScrollRig(camera, lookAt) {
  gsap.to(camera.position, {
    x: 4, y: 2.5, z: 9,
    ease: 'none',
    scrollTrigger: {
      trigger: '#pipeline',
      start: 'top bottom',
      end: 'bottom top',
      scrub: 1,
      onUpdate: () => camera.lookAt(lookAt),
    },
  });
}
```

Rules: scrub values at or above 1 for calm motion, one ScrollTrigger per property,
kill triggers on page hide, never pin the body on mobile widths.

## Pattern 3: React Three Fiber inside the Remotion video

Use for 3D dressing in the rendered video only. The video project builds with
Remotion, so React Three Fiber components compose naturally with the timeline.

```jsx
// video/src/Scene3D.jsx (new file, example shape)
import { Canvas } from '@react-three/fiber';

export function Scene3D() {
  return (
    <Canvas frameloop="demand" dpr={[1, 2]} gl={{ antialias: true, alpha: true }}>
      <ambientLight intensity={0.6} />
      <directionalLight position={[5, 8, 4]} />
      <mesh>
        <icosahedronGeometry args={[1.4, 1]} />
        <meshStandardMaterial color="#5266eb" roughness={0.35} />
      </mesh>
    </Canvas>
  );
}
```

Rules: frameloop demand so frames render on timeline ticks, dpr capped at 2,
no OrbitControls or browser-only globals (Remotion renders headless), keep the
cobalt accent as the only saturated colour to match the page system.

## Performance checklist

- One WebGL context per page, pixel ratio capped at 2, demand rendering outside heroes.
- Dispose geometries, materials and renderers on unmount; remove listeners.
- No layout thrash: read sizes in resize handlers, write transforms in the loop.
- Test at 390 px width and with throttled CPU before merging.
- Confirm `make site` still serves a page under about 12 MB after adding libraries.

## Pitfalls

- Two libraries animating one property fight each other. Give position to GSAP and
  scale or opacity to springs, never both to the same field.
- React state that mirrors the scene at 60 fps re-renders the tree to death. Keep
  per-frame values in refs, commit to state only on gestures.
- ScrollTrigger with pinning breaks mobile reading flow. Prefer scrubbed transforms
  with no pinning on this site.
- Autoplay policies block audio, not WebGL. Still, start heavy scenes on first
  interaction or intersection, not on load.

## Decision matrix

- Marketing hero with ambient motion: Three.js plus pointer parallax, no framework.
- Scroll-linked camera move: Three.js plus GSAP ScrollTrigger.
- Interactive product viewer in React: React Three Fiber plus Motion.
- Timeline-driven 3D in the video: React Three Fiber inside the Remotion project.
- Physics-feel gestures: React Three Fiber plus React Spring, separate properties.

## Sources

- Upstream skill: https://github.com/freshtechbro/claudedesignskills/blob/main/.claude/skills/web3d-integration-patterns/SKILL.md
- Page system: site/DESIGN.md
- Video project: video/ via `make video` and `make render`
- Page preview: `make site`

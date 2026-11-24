/* melodias-studio hero 3D: one ambient particle wave behind the headline.
   Three.js only, no framework. Static frame when the user prefers reduced
   motion, paused while the hero is off screen. Never throws: any failure
   leaves the static hero behind it. */

import * as THREE from "three";

export function initHero3D(canvas) {
  var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var mobile = window.innerWidth < 760;

  var renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas: canvas, antialias: true, alpha: true });
  } catch (err) {
    return function () {};
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, mobile ? 1.5 : 2));
  renderer.setClearColor(0x000000, 0);

  var scene = new THREE.Scene();
  var camera = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
  camera.position.set(0, 1.2, 8);

  var cols = mobile ? 56 : 90;
  var rows = mobile ? 30 : 50;
  var count = cols * rows;
  var positions = new Float32Array(count * 3);
  var colors = new Float32Array(count * 3);
  var cobalt = new THREE.Color("#5266eb");
  var ivory = new THREE.Color("#ededf3");
  var seeds = new Float32Array(count);

  var i = 0;
  for (var x = 0; x < cols; x++) {
    for (var y = 0; y < rows; y++) {
      var px = (x / (cols - 1) - 0.5) * 16;
      var py = (y / (rows - 1) - 0.5) * 9;
      positions[i * 3] = px;
      positions[i * 3 + 1] = py;
      positions[i * 3 + 2] = 0;
      var accent = (x * 7 + y * 13) % 29 === 0;
      var c = accent ? cobalt : ivory;
      var dim = accent ? 0.9 : 0.28 + ((x * 3 + y * 5) % 7) / 7 * 0.25;
      colors[i * 3] = c.r * dim;
      colors[i * 3 + 1] = c.g * dim;
      colors[i * 3 + 2] = c.b * dim;
      seeds[i] = (x * 0.7 + y * 1.3) * 0.35;
      i++;
    }
  }

  var geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));

  var material = new THREE.PointsMaterial({
    size: 0.045,
    vertexColors: true,
    transparent: true,
    opacity: 0.85,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    sizeAttenuation: true,
  });

  var points = new THREE.Points(geometry, material);
  points.rotation.x = -0.25;
  scene.add(points);

  var target = { x: 0, y: 0 };
  var onPointer = function (event) {
    target.x = (event.clientX / window.innerWidth - 0.5) * 0.7;
    target.y = (event.clientY / window.innerHeight - 0.5) * 0.45;
  };
  window.addEventListener("pointermove", onPointer, { passive: true });

  function resize() {
    var w = canvas.clientWidth || window.innerWidth;
    var h = canvas.clientHeight || 480;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  window.addEventListener("resize", resize);
  resize();

  function draw(time) {
    var pos = geometry.attributes.position.array;
    for (var k = 0; k < count; k++) {
      pos[k * 3 + 2] = Math.sin(time * 0.0006 + seeds[k]) * 0.55;
    }
    geometry.attributes.position.needsUpdate = true;
    points.rotation.y += (target.x * 0.35 - points.rotation.y) * 0.04;
    points.rotation.x += (-0.25 + target.y * 0.25 - points.rotation.x) * 0.04;
    var fade = Math.max(0, 1 - window.scrollY / (window.innerHeight * 1.2));
    material.opacity = 0.85 * fade;
    renderer.render(scene, camera);
  }

  var running = false;
  function frame(time) {
    if (!running) return;
    draw(time || 0);
    requestAnimationFrame(frame);
  }

  function dispose() {
    running = false;
    window.removeEventListener("pointermove", onPointer);
    window.removeEventListener("resize", resize);
    geometry.dispose();
    material.dispose();
    renderer.dispose();
  }

  if (reduced) {
    draw(1200);
    window.addEventListener("resize", function () {
      resize();
      draw(1200);
    });
    return dispose;
  }

  running = true;
  requestAnimationFrame(frame);

  if ("IntersectionObserver" in window) {
    new IntersectionObserver(function (entries) {
      var visible = entries[0].isIntersecting && !document.hidden;
      if (visible && !running) {
        running = true;
        requestAnimationFrame(frame);
      } else if (!visible) {
        running = false;
      }
    }).observe(canvas);
  }

  document.addEventListener("visibilitychange", function () {
    if (document.hidden) {
      running = false;
    } else if (!reduced) {
      running = true;
      requestAnimationFrame(frame);
    }
  });

  return dispose;
}

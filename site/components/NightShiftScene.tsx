'use client';

import { useEffect, useRef } from 'react';
import s from './NightShift.module.css';

const vertex = `attribute vec2 position; varying vec2 uv;
void main() { uv = position * .5 + .5; gl_Position = vec4(position, 0., 1.); }`;
const fragment = `precision mediump float;
uniform sampler2D scene; uniform vec2 resolution; uniform vec2 imageSize;
uniform vec2 camera; uniform float time; uniform float motion;
varying vec2 uv;
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1,311.7))) * 43758.5453); }
float rain(vec2 p, float density, float speed) {
  vec2 grid = p * vec2(density, density * .45);
  grid.x += p.y * 2.2;
  vec2 cell = floor(grid); float seed = hash(vec2(cell.x, 5.));
  float y = fract(grid.y + time * speed * (1. + seed));
  float x = abs(fract(grid.x) - (.2 + seed * .6));
  return (1. - smoothstep(.005, .033, x)) * smoothstep(.55,.8,y) * (1. - smoothstep(.94,1.,y));
}
void main() {
  float ratio = resolution.x / resolution.y, imageRatio = imageSize.x / imageSize.y;
  vec2 cover = ratio > imageRatio ? vec2(1., imageRatio / ratio) : vec2(ratio / imageRatio, 1.);
  vec2 p = (uv - .5) * cover + .5;
  // The desk is nearer than the window; each responds at a different camera depth.
  float depth = .15 + .85 * (1. - smoothstep(.12,.65,p.y));
  p += camera * vec2(.013,.009) * depth * motion;
  vec3 color = texture2D(scene, clamp(p, .002, .998)).rgb;
  float windowMask = smoothstep(.67,.72,p.x) * smoothstep(.30,.38,p.y);
  float rainLight = rain(p, 61., 1.3) * .16 + rain(p + .31, 97., 1.7) * .08;
  color += vec3(.40,.62,.68) * rainLight * windowMask * motion;
  vec2 lamp = (p - vec2(.885,.44)) / vec2(.18,.23);
  float glow = exp(-dot(lamp,lamp) * 2.);
  float flicker = .94 + .035 * sin(time * 2.1) + .018 * sin(time * 6.7) + .008 * sin(time * 11.3);
  color += vec3(1.,.61,.24) * glow * (.028 + .04 * flicker * motion);
  float mist = .004 * sin(time * .25 + p.x * 4.) * motion;
  color += vec3(.3,.5,.5) * mist;
  gl_FragColor = vec4(color,1.);
}`;

export function NightShiftScene({ active }: { active: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const activeRef = useRef(active);
  useEffect(() => { activeRef.current = active; }, [active]);

  useEffect(() => {
    const surface = canvas.current!;
    const host = surface.parentElement!;
    const gl = surface.getContext('webgl', { alpha: false, antialias: false, powerPreference: 'low-power', preserveDrawingBuffer: true })!;
    if (!gl) return;
    const shaders: WebGLShader[] = [];
    const program = gl.createProgram()!;
    const buffer = gl.createBuffer()!;
    const texture = gl.createTexture()!;
    let frame = 0, loaded = false, disposed = false, visible = false, lastDraw = 0, elapsed = 0;
    let last = performance.now(), cameraX = 0, cameraY = 0, targetX = 0, targetY = 0;
    const reduced = matchMedia('(prefers-reduced-motion: reduce)');
    try {
      for (const [type, source] of [[gl.VERTEX_SHADER, vertex], [gl.FRAGMENT_SHADER, fragment]] as const) {
        const shader = gl.createShader(type)!;
        shaders.push(shader); gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error('Scene shader could not compile');
        gl.attachShader(program, shader);
      }
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error('Scene shader could not link');
    } catch {
      shaders.forEach((shader) => gl.deleteShader(shader)); gl.deleteProgram(program); gl.deleteBuffer(buffer); gl.deleteTexture(texture);
      return;
    }
    gl.useProgram(program); gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
    const position = gl.getAttribLocation(program, 'position');
    gl.enableVertexAttribArray(position); gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    const uniforms = Object.fromEntries(['scene','resolution','imageSize','camera','time','motion'].map((name) => [name, gl.getUniformLocation(program, name)]));
    gl.uniform1i(uniforms.scene, 0);
    const image = new window.Image();
    image.onload = () => {
      if (disposed || gl.isContextLost()) return;
      gl.bindTexture(gl.TEXTURE_2D, texture); gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, image);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.uniform2f(uniforms.imageSize, image.naturalWidth, image.naturalHeight);
      loaded = true; surface.dataset.renderer = 'webgl'; surface.style.opacity = '1';
    };
    image.src = '/history/night-shift.webp';
    const resize = new ResizeObserver(() => {
      const bounds = host.getBoundingClientRect(), dpr = Math.min(devicePixelRatio || 1, 1.5);
      surface.width = Math.round(bounds.width * dpr); surface.height = Math.round(bounds.height * dpr);
      gl.viewport(0, 0, surface.width, surface.height); gl.uniform2f(uniforms.resolution, surface.width, surface.height);
      lastDraw = 0;
    });
    resize.observe(host);
    const observer = new IntersectionObserver(([entry]) => { visible = entry.isIntersecting; });
    observer.observe(host);
    const pointer = (event: PointerEvent) => {
      const bounds = host.getBoundingClientRect();
      targetX = (event.clientX - bounds.left) / bounds.width * 2 - 1;
      targetY = 1 - (event.clientY - bounds.top) / bounds.height * 2;
    };
    const leave = () => { targetX = 0; targetY = 0; };
    host.addEventListener('pointermove', pointer); host.addEventListener('pointerleave', leave);
    const lost = (event: Event) => { event.preventDefault(); loaded = false; surface.style.opacity = '0'; surface.dataset.renderer = 'static'; };
    surface.addEventListener('webglcontextlost', lost);
    function draw(now: number) {
      frame = requestAnimationFrame(draw);
      const delta = Math.min((now - last) / 1000, .1); last = now;
      if (!loaded || !visible || document.hidden || gl.isContextLost()) return;
      if (activeRef.current && !reduced.matches) elapsed += delta;
      if (now - lastDraw < 33) return;
      // Idle/paused scenes need a single static frame, not a perpetual GPU draw.
      if ((!activeRef.current || reduced.matches) && lastDraw !== 0) return;
      lastDraw = now;
      cameraX += (targetX - cameraX) * .045; cameraY += (targetY - cameraY) * .045;
      gl.uniform2f(uniforms.camera, cameraX + Math.sin(elapsed * .13) * .12, cameraY);
      gl.uniform1f(uniforms.time, elapsed); gl.uniform1f(uniforms.motion, reduced.matches ? 0 : 1);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
    }
    frame = requestAnimationFrame(draw);
    return () => {
      disposed = true; cancelAnimationFrame(frame); resize.disconnect(); observer.disconnect(); image.onload = null;
      host.removeEventListener('pointermove', pointer); host.removeEventListener('pointerleave', leave); surface.removeEventListener('webglcontextlost', lost);
      shaders.forEach((shader) => gl.deleteShader(shader)); gl.deleteProgram(program); gl.deleteBuffer(buffer); gl.deleteTexture(texture);
    };
  }, []);
  return <canvas ref={canvas} className={s.sceneCanvas} aria-hidden="true" data-renderer="static" />;
}

// A self-contained WebGL flowing color field: no remote shader service or runtime assets.
export function mountTerrain(canvas: HTMLCanvasElement): () => void {
  const gl = canvas.getContext('webgl', { alpha: true, antialias: false, powerPreference: 'low-power' })
  if (!gl) return () => {}
  const shaders: WebGLShader[] = []
  const compile = (type: number, source: string) => {
    const shader = gl.createShader(type)
    if (!shader) return null
    shaders.push(shader)
    gl.shaderSource(shader, source); gl.compileShader(shader)
    return gl.getShaderParameter(shader, gl.COMPILE_STATUS) ? shader : null
  }
  const vertex = compile(gl.VERTEX_SHADER, 'attribute vec2 position; void main(){gl_Position=vec4(position,0.,1.);}')
  const fragment = compile(gl.FRAGMENT_SHADER, `precision mediump float;
    uniform vec2 resolution; uniform float time; uniform float darkMode;
    float hash(vec2 p) { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
    float noise(vec2 p) {
      vec2 i=floor(p), f=fract(p); f=f*f*(3.-2.*f);
      return mix(mix(hash(i),hash(i+vec2(1.,0.)),f.x),mix(hash(i+vec2(0.,1.)),hash(i+vec2(1.)),f.x),f.y);
    }
    float field(vec2 p) {
      float v=0., a=.5;
      for(int i=0;i<3;i++){v+=a*noise(p);p=mat2(.8,-.6,.6,.8)*p*2.02+3.1;a*=.5;}
      return v;
    }
    void main(){
      vec2 uv=gl_FragCoord.xy/resolution;
      vec2 p=uv*vec2(2.4,1.8);
      float t=time*.045;
      vec2 warp=vec2(field(p+vec2(t,-t*.4)),field(p+vec2(5.2,-3.7)+t*.6));
      float flow=field(p+warp*2.0+vec2(-t*.35,t*.2));
      vec3 ice=mix(vec3(.91,.97,1.),vec3(.035,.075,.15),darkMode);
      vec3 sky=mix(vec3(.40,.76,.98),vec3(.08,.22,.42),darkMode);
      vec3 cobalt=mix(vec3(.22,.48,.92),vec3(.13,.30,.61),darkMode);
      vec3 mist=mix(vec3(.68,.91,.98),vec3(.055,.16,.27),darkMode);
      vec3 color=mix(ice,sky,smoothstep(.24,.66,flow));
      float ribbon=sin(uv.y*7.+warp.x*3.8+uv.x*2.5+t*.35);
      color=mix(color,mist,smoothstep(-.2,.8,ribbon)*.75);
      color=mix(color,cobalt,smoothstep(.52,.78,flow)*smoothstep(.12,.95,uv.x)*.7);
      float glow=exp(-pow((uv.y-.34-warp.y*.18)*5.5,2.));
      color=mix(color,ice,glow*.78);
      // A quiet reading area maintains contrast in both palettes.
      color=mix(color,ice,(1.-smoothstep(.0,.72,uv.x))*.25);
      float grain=(hash(gl_FragCoord.xy)-.5)/255.;
      gl_FragColor=vec4(color+grain,1.);
    }`)
  const program = gl.createProgram()
  if (!vertex || !fragment || !program) { shaders.forEach(s => gl.deleteShader(s)); if (program) gl.deleteProgram(program); return () => {} }
  gl.attachShader(program, vertex); gl.attachShader(program, fragment); gl.linkProgram(program)
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) { shaders.forEach(s => gl.deleteShader(s)); gl.deleteProgram(program); return () => {} }
  gl.useProgram(program)
  const buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW)
  const position = gl.getAttribLocation(program, 'position')
  gl.enableVertexAttribArray(position); gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0)
  const resolution = gl.getUniformLocation(program, 'resolution'), time = gl.getUniformLocation(program, 'time')
  const darkMode = gl.getUniformLocation(program, 'darkMode')
  const motion = matchMedia('(prefers-reduced-motion: reduce)')
  let frame = 0, visible = true, lost = false, last = 0
  const draw = (now: number) => {
    if (lost) return
    gl.uniform1f(darkMode, document.documentElement.classList.contains('dark') ? 1 : 0)
    gl.uniform1f(time, motion.matches ? 0 : now / 1000); gl.drawArrays(gl.TRIANGLES, 0, 6)
  }
  const tick = (now: number) => {
    frame = 0
    if (!visible || document.hidden || motion.matches || lost) return
    if (now - last > 40) { draw(now); last = now }
    frame = requestAnimationFrame(tick)
  }
  const sync = () => { cancelAnimationFrame(frame); frame = 0; if (visible && !document.hidden) { draw(performance.now()); if (!motion.matches && !lost) frame = requestAnimationFrame(tick) } }
  const themeObserver = new MutationObserver(sync)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
  const resize = new ResizeObserver(() => {
    const ratio = Math.min(devicePixelRatio, 1.5)
    canvas.width = Math.max(1, Math.round(canvas.clientWidth * ratio)); canvas.height = Math.max(1, Math.round(canvas.clientHeight * ratio))
    gl.viewport(0, 0, canvas.width, canvas.height); gl.uniform2f(resolution, canvas.width, canvas.height); sync()
  })
  const observer = new IntersectionObserver(entries => { visible = entries[0].isIntersecting; sync() })
  const contextLost = () => { lost = true; cancelAnimationFrame(frame) }
  canvas.addEventListener('webglcontextlost', contextLost)
  resize.observe(canvas); observer.observe(canvas)
  motion.addEventListener('change', sync); document.addEventListener('visibilitychange', sync)
  return () => { cancelAnimationFrame(frame); themeObserver.disconnect(); resize.disconnect(); observer.disconnect(); motion.removeEventListener('change', sync); document.removeEventListener('visibilitychange', sync); canvas.removeEventListener('webglcontextlost', contextLost); gl.deleteBuffer(buffer); gl.deleteProgram(program); shaders.forEach(s => gl.deleteShader(s)) }
}

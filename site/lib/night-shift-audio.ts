// Mechanical impacts, a motor, rain and a metal bell, synthesized locally.
export class TelexAudio {
  private context = new AudioContext();
  private master = this.context.createGain();
  private noise: AudioBuffer;
  private enabled = false;
  private muteTimer: ReturnType<typeof setTimeout> | null = null;
  private nextPrint = [0, 0, 0];
  private lines: number[] = [];
  private scheduler: ReturnType<typeof setInterval>;
  private ambientSources: (AudioBufferSourceNode | OscillatorNode)[] = [];

  constructor() {
    this.master.gain.value = 0; this.master.connect(this.context.destination);
    this.noise = this.context.createBuffer(1, this.context.sampleRate * 2, this.context.sampleRate);
    const samples = this.noise.getChannelData(0);
    for (let i = 0; i < samples.length; i++) samples[i] = Math.random() * 2 - 1;
    const rain = this.context.createBufferSource(), filter = this.context.createBiquadFilter(), rainGain = this.context.createGain();
    rain.buffer = this.noise; rain.loop = true; filter.type = 'lowpass'; filter.frequency.value = 1400; rainGain.gain.value = .055;
    rain.connect(filter); filter.connect(rainGain); rainGain.connect(this.master); rain.start(); this.ambientSources.push(rain);
    for (const frequency of [50, 101]) {
      const motor = this.context.createOscillator(), gain = this.context.createGain();
      motor.frequency.value = frequency; gain.gain.value = frequency === 50 ? .013 : .004;
      motor.connect(gain); gain.connect(this.master); motor.start(); this.ambientSources.push(motor);
    }
    this.scheduler = setInterval(() => {
      if (!this.enabled || this.context.state !== 'running') return;
      const now = this.context.currentTime;
      for (const line of this.lines) {
        if (this.nextPrint[line] < now) this.nextPrint[line] = now;
        while (this.nextPrint[line] < now + .075) {
          this.clack(this.nextPrint[line], (line - 1) * .45, .55);
          this.nextPrint[line] += .15;
        }
      }
    }, 50);
  }

  setActivity(enabled: boolean, lines: number[]) {
    this.lines = lines;
    if (enabled === this.enabled) return;
    this.enabled = enabled;
    if (this.muteTimer !== null) { clearTimeout(this.muteTimer); this.muteTimer = null; }
    if (enabled) void this.context.resume().catch(() => {});
    this.master.gain.cancelScheduledValues(this.context.currentTime);
    this.master.gain.setTargetAtTime(enabled ? .7 : 0, this.context.currentTime, .04);
    if (!enabled) {
      this.nextPrint.fill(0);
      this.muteTimer = setTimeout(() => { if (!this.enabled) void this.context.suspend().catch(() => {}); }, 250);
    }
  }

  key() { if (this.enabled) this.clack(this.context.currentTime, -.12, .85); }

  private clack(at: number, pan: number, strength: number) {
    const strike = this.context.createBufferSource(), filter = this.context.createBiquadFilter();
    const gain = this.context.createGain(), stereo = this.context.createStereoPanner();
    strike.buffer = this.noise; filter.type = 'bandpass'; filter.frequency.value = 1500 + Math.random() * 900; filter.Q.value = .7;
    gain.gain.setValueAtTime(.24 * strength, at); gain.gain.exponentialRampToValueAtTime(.001, at + .022);
    stereo.pan.value = pan; strike.connect(filter); filter.connect(gain); gain.connect(stereo); stereo.connect(this.master);
    strike.start(at, Math.random(), .04);
    strike.onended = () => { strike.disconnect(); filter.disconnect(); gain.disconnect(); stereo.disconnect(); };
    for (const [frequency, amplitude] of [[155, .04], [340, .017]]) {
      const body = this.context.createOscillator(), bodyGain = this.context.createGain();
      body.frequency.value = frequency + Math.random() * 25; bodyGain.gain.setValueAtTime(amplitude * strength, at);
      bodyGain.gain.exponentialRampToValueAtTime(.001, at + .055);
      body.connect(bodyGain); bodyGain.connect(this.master); body.start(at); body.stop(at + .06);
      body.onended = () => { body.disconnect(); bodyGain.disconnect(); };
    }
  }

  bell() {
    if (!this.enabled) return;
    const at = this.context.currentTime;
    for (const [frequency, amplitude] of [[840,.07], [1313,.028], [2194,.011]]) {
      const partial = this.context.createOscillator(), gain = this.context.createGain();
      partial.frequency.value = frequency; gain.gain.setValueAtTime(amplitude, at); gain.gain.exponentialRampToValueAtTime(.0001, at + .65);
      partial.connect(gain); gain.connect(this.master); partial.start(at); partial.stop(at + .7);
      partial.onended = () => { partial.disconnect(); gain.disconnect(); };
    }
  }

  dispose() { clearInterval(this.scheduler); if (this.muteTimer !== null) clearTimeout(this.muteTimer); this.ambientSources.forEach((source) => source.stop()); void this.context.close().catch(() => {}); }
}

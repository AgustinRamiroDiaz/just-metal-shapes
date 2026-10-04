// Headless, muted probe of the Web build: title -> level select -> lobby -> level.
// Prints the console and the RMS level of everything the page routes to the audio
// output (the tab is muted, the analyser still sees the signal), and saves title.png
// and level.png. Usage: node probe.js [url] [out_dir]
// Set CHROME_BIN to use a specific Chromium (for example a Playwright headless shell);
// otherwise Playwright's own browser is used (`npx playwright install chromium`).
const { chromium } = require('playwright-core');

const URL = process.argv[2] || 'http://127.0.0.1:8061/index.html';
const OUT = process.argv[3] || '/tmp/jms_web_probe';

const tap = () => {
  const Orig = window.AudioContext;
  window.AudioContext = class extends Orig {
    constructor(...args) {
      super(...args);
      window.__ctx = this;
      this.__analyser = super.createAnalyser();
      this.__analyser.fftSize = 2048;
    }
  };
  const connect = AudioNode.prototype.connect;
  AudioNode.prototype.connect = function (dest, ...rest) {
    const ctx = this.context;
    if (dest === ctx.destination && ctx.__analyser && this !== ctx.__analyser) {
      connect.call(this, ctx.__analyser);
    }
    return connect.call(this, dest, ...rest);
  };
  window.__rms = () => {
    const ctx = window.__ctx;
    if (!ctx || !ctx.__analyser) return { state: 'none' };
    const data = new Float32Array(ctx.__analyser.fftSize);
    ctx.__analyser.getFloatTimeDomainData(data);
    let sum = 0;
    for (const v of data) sum += v * v;
    return { state: ctx.state, time: ctx.currentTime.toFixed(2), rms: Math.sqrt(sum / data.length).toFixed(4) };
  };
};

(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.CHROME_BIN || undefined,
    args: ['--mute-audio', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const logs = [];
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  await page.addInitScript(tap);
  await page.goto(URL);
  await page.waitForFunction(() => document.querySelector('#status') == null
    || getComputedStyle(document.querySelector('#status')).display === 'none' || window.__ctx, null, { timeout: 120000 }).catch(() => {});
  await page.waitForTimeout(8000);
  const sample = async (label) => {
    const values = [];
    for (let i = 0; i < 5; i++) {
      values.push(await page.evaluate(() => window.__rms()));
      await page.waitForTimeout(300);
    }
    console.log(label, JSON.stringify(values));
  };
  await page.mouse.click(640, 360);
  await page.waitForTimeout(2000);
  await sample('title');
  await page.screenshot({ path: `${OUT}/title.png` });
  await page.keyboard.press('Enter');
  await page.waitForTimeout(2500);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(2500);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(800);
  await page.keyboard.down('Enter');
  await page.waitForTimeout(2500);
  await page.keyboard.up('Enter');
  await page.waitForTimeout(7000);
  await sample('level');
  await page.screenshot({ path: `${OUT}/level.png` });
  await page.waitForTimeout(6000);
  await sample('level+6s');
  console.log('--- console ---');
  for (const line of logs) console.log(line);
  await browser.close();
})();

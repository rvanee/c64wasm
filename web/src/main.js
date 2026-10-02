// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The page: wires the emulator to the desk scene, the controls and the
// dialogs, and runs the frame loop.

import { Sound } from './audio/sound.js';
import { CrtRenderer } from './display/crt-renderer.js';
import { DriveLcd, LCD_COLUMNS, petscii } from './display/drive-lcd.js';
import { Led } from './display/led.js';
import { Monitor2002 } from './display/monitor-2002.js';
import { Scene } from './display/scene.js';
import { attachKeyboard } from './input/keyboard.js';
import { Typer } from './input/typer.js';
import { Emulator } from './machine/emulator.js';
import { RomSet } from './machine/rom-set.js';
import { DiskSlot } from './media/disk-slot.js';
import { save, settings } from './settings.js';
import { $, download, flash, readFile } from './ui/dom.js';
import { RomManager } from './ui/rom-manager.js';
import { SettingsDialog } from './ui/settings-dialog.js';
import { WelcomeDialog } from './ui/welcome-dialog.js';

const SPEAKER_ON = '<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M3 9v6h4l5 4V5L7 9H3zm13.5 3a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4zM14 3.2v2.1a7 7 0 0 1 0 13.4v2.1a9 9 0 0 0 0-17.6z"/></svg>';
const SPEAKER_OFF = '<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M3 9v6h4l5 4V5L7 9H3zm18.3-.3-1.4-1.4L17 10.2l-2.9-2.9-1.4 1.4 2.9 2.9-2.9 2.9 1.4 1.4 2.9-2.9 2.9 2.9 1.4-1.4-2.9-2.9z"/></svg>';

const stage = $('stage');
const emulator = new Emulator();
const romSet = new RomSet();
const sound = new Sound();
const disks = new DiskSlot(emulator, () => updateDiskUi());
const leds = {
  c64: new Led('c64'),
  drivePower: new Led('power'),
  driveActivity: new Led('act'),
  monitor: new Led('monitor'),
};
const driveLcd = new DriveLcd([$('driveLcd'), $('driveLcdZoom')], () => romSet.get('chargen'));
let renderer, scene, monitor, settingsDialog, welcome;
let monitorOn = true; // the 2002's own power switch
let waitingForRoms = false; // switch on once the ROMs are there

// ---- power ----------------------------------------------------------------

async function powerOn() {
  if (emulator.on) return;
  if (!emulator.demo && !romSet.ready) { waitingForRoms = true; welcome.open(); return; }
  waitingForRoms = false;
  const problems = await emulator.powerOn(romSet.forMachine(), {
    ntsc: settings.video === 'ntsc',
    drive: settings.driveOn,
    deviceNumber: settings.deviceNumber,
    sid8580: settings.sid === '8580',
    audioRate: sound.sampleRate,
    disk: disks.disk?.bytes,
  });
  for (const p of problems) flash(p, 6000);
  if (settings.video === 'ntsc') { renderer.setWindow(58, 6, 402, 240); renderer.setSubcarrier(0.4375); monitor.setFrameLines(263); }
  else { renderer.setWindow(50, 7, 402, 287); renderer.setSubcarrier(0.5625); monitor.setFrameLines(312); }
  applyMonitorPower();
  $('powerBtn').classList.add('on');
  updateDiskUi();
  stage.focus();
}

function powerOff() {
  if (!emulator.on) return;
  disks.capture();
  typer.cancel(emulator.machine);
  emulator.powerOff();
  sound.flush();
  sound.drive?.setMotor(false);
  applyMonitorPower();
  $('powerBtn').classList.remove('on');
  updateDiskUi();
}

let cycling = false;
function powerCycle() {
  if (cycling) return;
  powerOff();
  cycling = true;
  setTimeout(() => { cycling = false; powerOn(); }, 450);
}

/// The picture shows when both the computer and the monitor are on.
function applyMonitorPower() {
  renderer.setPower(emulator.on && monitorOn);
}

$('powerBtn').addEventListener('click', () => (emulator.on ? powerOff() : powerOn()));
$('hotMonitorPower').addEventListener('click', () => { monitorOn = !monitorOn; applyMonitorPower(); });

// ---- disks ----------------------------------------------------------------

function updateDiskUi() {
  $('diskName').textContent = disks.disk ? `in drive: ${disks.disk.name}` : 'no disk';
  $('ejectBtn').disabled = !disks.disk;
  $('downloadBtn').disabled = !disks.disk && !disks.lastEjected;
}

function insertDisk(name, bytes) {
  try { disks.insert(name, bytes); } catch (e) { flash(e.message); return; }
  flash(emulator.driveAttached || !emulator.on ? `Inserted ${name}` : `${name} is ready, but no 1541 is connected (Settings)`);
}

const diskInput = $('diskFile');
const pickDisk = () => diskInput.click();
for (const id of ['insertBtn', 'hotDrive', 'hotDisks']) $(id).addEventListener('click', pickDisk);
diskInput.addEventListener('change', async () => {
  const f = diskInput.files[0];
  diskInput.value = '';
  if (f) insertDisk(f.name, await readFile(f));
});
document.querySelectorAll('a[data-disk]').forEach(a => a.addEventListener('click', async (e) => {
  e.preventDefault();
  try {
    const resp = await fetch(a.getAttribute('href'));
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    insertDisk(a.getAttribute('href').split('/').pop(), new Uint8Array(await resp.arrayBuffer()));
  } catch (err) { flash(`Could not load ${a.textContent}: ${err.message}`); }
}));
$('ejectBtn').addEventListener('click', () => {
  const d = disks.eject();
  if (d) flash(`Ejected ${d.name}`);
});
$('downloadBtn').addEventListener('click', () => {
  const d = disks.forDownload();
  if (d) download(d.bytes, d.name);
});
stage.addEventListener('dragover', (e) => { e.preventDefault(); stage.classList.add('dragging'); });
stage.addEventListener('dragleave', () => stage.classList.remove('dragging'));
stage.addEventListener('drop', async (e) => {
  e.preventDefault();
  stage.classList.remove('dragging');
  const f = e.dataTransfer.files[0];
  if (!f) return;
  if (/\.(bas|txt|asc)$/i.test(f.name)) { basicText = await f.text(); startTyping(); return; }
  insertDisk(f.name, await readFile(f));
});

// ---- typing a BASIC listing -------------------------------------------------

let basicText = null;
const typer = new Typer({
  onStatus: (text) => { $('typeStatus').textContent = text; },
  onDone: () => { $('typeFileBtn').disabled = !basicText; $('cancelTypeBtn').hidden = true; },
});
function startTyping() {
  if (!emulator.on) { flash('Switch the computer on first'); return; }
  typer.start(basicText, emulator.totalCycles);
  $('typeFileBtn').disabled = true;
  $('cancelTypeBtn').hidden = false;
}
$('basicFile').addEventListener('change', async (e) => {
  const f = e.target.files[0];
  typer.cancel(emulator.machine);
  basicText = f ? await f.text() : null;
  $('typeFileBtn').disabled = !basicText;
  if (f) $('typeStatus').textContent = `loaded ${f.name}: click "Type it".`;
});
$('typeFileBtn').addEventListener('click', () => basicText && startTyping());
$('cancelTypeBtn').addEventListener('click', () => typer.cancel(emulator.machine, 'cancelled.'));

// ---- keyboard ---------------------------------------------------------------

attachKeyboard(stage, () => emulator.machine);
stage.addEventListener('mousedown', (e) => {
  if (!e.target.closest('button, .knob, #monitorPanel')) setTimeout(() => stage.focus(), 0);
});

// ---- sound ------------------------------------------------------------------

async function startSound() {
  if (emulator.demo) { flash('No sound in the picture demo: the emulator is not built'); return; }
  try {
    await sound.start(settings);
    emulator.machine?.set_audio_rate(sound.sampleRate);
  } catch (e) {
    flash(`Sound unavailable: ${e.message}`);
  }
}

function stopSound() {
  sound.stop();
  emulator.machine?.set_audio_rate(0);
}

function updateVolumeUi() {
  const muted = settings.volume <= 0;
  $('soundBtn').innerHTML = muted ? SPEAKER_OFF : SPEAKER_ON;
  $('soundBtn').title = muted ? 'Unmute' : 'Mute';
  $('volume').value = settings.volume;
  $('volume').title = muted ? 'Muted' : `Volume ${Math.round(settings.volume * 100)}%`;
}

/// One control for sound: all the way down is mute; anything above starts
/// the audio (moving a slider counts as the click browsers require).
async function setVolume(v) {
  settings.volume = v;
  if (v > 0) settings.lastVolume = v;
  save();
  updateVolumeUi();
  monitor?.sync();
  if (v > 0) {
    if (!sound.on) await startSound();
    sound.setVolume(v);
  } else if (sound.on) stopSound();
}

$('soundBtn').addEventListener('click', () => setVolume(settings.volume > 0 ? 0 : settings.lastVolume || 0.7));
$('volume').addEventListener('input', () => setVolume(+$('volume').value));
for (const type of ['pointerdown', 'keydown']) {
  stage.addEventListener(type, () => { if (!emulator.demo && settings.volume > 0 && !sound.on) startSound(); });
}

// ---- the 2002's control door ---------------------------------------------------

const monitorPanel = $('monitorPanel');
function openMonitorControls() {
  monitor.sync();
  monitorPanel.hidden = false;
  monitorPanel.querySelector('.dial').focus();
}
function closeMonitorControls() {
  monitorPanel.hidden = true;
  stage.focus();
}
$('hotControls').addEventListener('click', () => (monitorPanel.hidden ? openMonitorControls() : closeMonitorControls()));
$('monitorPanelClose').addEventListener('click', closeMonitorControls);
monitorPanel.addEventListener('keydown', (e) => { if (e.key === 'Escape') { e.stopPropagation(); closeMonitorControls(); } });

// ---- indicators -------------------------------------------------------------------

let activity = 0;
let motorWas = false;

function updateIndicators() {
  const m = emulator.machine;
  const drive = !!m && emulator.driveAttached;
  leds.c64.set(m ? 1 : 0);
  leds.monitor.set(m && monitorOn ? 1 : 0);
  leds.drivePower.set(drive ? 1 : 0);
  activity = drive ? Math.max(m.drive_led(0), activity * 0.35) : 0;
  leds.driveActivity.set(activity);

  const status = $('driveStatus');
  if (drive) {
    const t = m.drive_track(0);
    const device = emulator.demo ? 8 : settings.deviceNumber;
    status.textContent = `1541 #${device}: track ${t % 1 ? t.toFixed(1) : t}${m.drive_motor(0) ? ' · motor' : ''}${m.drive_has_disk(0) ? '' : ' · no disk'}`;
  } else status.textContent = m ? (settings.driveOn ? '1541: no DOS ROM' : '1541: disconnected') : '';

  // A write ends with the motor running on: re-read the label (a format or
  // rename may have changed it) once the motor stops.
  const motor = drive && m.drive_motor(0);
  if (motorWas && !motor) { disks.capture(); disks.refreshLabel(); }
  motorWas = motor;
  updateDriveLcd(drive);
}

function updateDriveLcd(drive) {
  const show = settings.driveLcd && settings.driveOn;
  $('driveLcd').hidden = !show;
  $('driveLcdZoom').hidden = !show;
  const { disk, label } = disks;
  let top, bottom;
  if (!disk) { top = petscii('    NO DISK'); bottom = new Uint8Array(0); }
  else if (!label) { top = petscii(disk.name.slice(0, LCD_COLUMNS)); bottom = petscii('ID ??'); }
  else {
    top = label.name;
    const track = drive ? `TRK ${String(Math.round(emulator.machine.drive_track(0))).padStart(2, ' ')}` : '';
    const id = Uint8Array.from([...petscii('ID '), ...label.id]);
    const pad = Math.max(1, LCD_COLUMNS - id.length - track.length);
    bottom = Uint8Array.from([...id, ...new Array(pad).fill(0x20), ...petscii(track)]);
  }
  if (show) driveLcd.show([top, bottom], drive);
}

// ---- frame loop -----------------------------------------------------------------------

let lastTime = 0, readoutCycles = 0, lastReadout = 0, jamShown = false;

function frame(t) {
  requestAnimationFrame(frame);
  const dt = lastTime ? Math.min(t - lastTime, 250) : 16.7;
  lastTime = t;
  const m = emulator.machine;
  if (m) {
    if (!emulator.demo && m.jammed()) {
      if (!jamShown) { flash('The CPU jammed on an illegal opcode: switch off and on again'); jamShown = true; }
    } else {
      jamShown = false;
      readoutCycles += emulator.run(dt, settings.speed === 'fast');
      if (typer.busy) typer.advance(m, emulator.totalCycles);
      if (!emulator.demo) sound.pump(m, emulator.driveAttached);
      const f = emulator.frame();
      renderer.setSource(f.rgba, f.width, f.height);
    }
  }
  monitor.tick(dt / 1000);
  updateIndicators();
  scene.updateSpill(t);
  renderer.render(t);
  if (t - lastReadout > 1000) {
    $('speedReadout').textContent = m ? `${(readoutCycles / ((t - lastReadout) / 1000) / 1e6).toFixed(2)} MHz${emulator.demo ? ' (demo picture)' : ''}` : '';
    readoutCycles = 0;
    lastReadout = t;
  }
}

// ---- start-up -------------------------------------------------------------------------

(async () => {
  const mask = new Image();
  mask.src = 'assets/glass-mask.png';
  await mask.decode();
  renderer = new CrtRenderer($('crt'), mask);
  $('glInfo').textContent = renderer.isWebGL ? '' : 'WebGL unavailable: using a simple 2D fallback.';
  scene = new Scene({ renderer, settings, save });
  monitor = new Monitor2002({
    renderer,
    panel: $('monitorPanel'),
    knobs: settings.monitor,
    volume: { get: () => settings.volume, set: (v) => setVolume(v) },
    connection: {
      get: () => settings.connection,
      set: (c) => { settings.connection = c; save(); renderer.setConnection(c); },
    },
    onChange: save,
  });
  const romManager = new RomManager({ romSet, onChanged: () => { if (emulator.on && romSet.ready) powerCycle(); } });
  settingsDialog = new SettingsDialog({
    settings, save, renderer, romManager,
    actions: {
      restart: powerCycle,
      display: () => { monitorOn = true; scene.applyDisplay(); applyMonitorPower(); },
      connection: () => { renderer.setConnection(settings.connection); monitor.sync(); },
      sid: () => {
        emulator.machine?.set_sid_8580(settings.sid === '8580');
        emulator.machine?.set_audio_rate(sound.sampleRate);
      },
      driveSounds: () => sound.setDriveSounds(settings.driveSounds),
      driveVolume: () => sound.setDriveVolume(settings.driveVolume),
      deviceNumber: () => { powerCycle(); flash(`The 1541 is now device ${settings.deviceNumber}: LOAD"$",${settings.deviceNumber}`); },
      monitorControls: openMonitorControls,
    },
  });
  $('settingsBtn').addEventListener('click', () => settingsDialog.open());
  $('settings').addEventListener('close', () => {
    stage.focus();
    if (waitingForRoms && romSet.ready) powerOn();
  });
  welcome = new WelcomeDialog({ romSet, onStart: powerOn, onManual: () => settingsDialog.open('roms') });

  scene.applyDisplay();
  updateDiskUi();
  updateVolumeUi();
  requestAnimationFrame(frame);
  await emulator.load();
  await romSet.load();
  if (emulator.demo) {
    flash('Demo picture: the emulator (web/pkg) is not built yet; see README.md', 8000);
    setTimeout(powerOn, 600);
  } else if (romSet.ready) setTimeout(powerOn, 400);
  else { waitingForRoms = true; welcome.open(); }
})();

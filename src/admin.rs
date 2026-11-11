pub const INDEX: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sentinel Streaming Admin</title>
  <style>
    :root { color-scheme: dark; font-family: system-ui, sans-serif; }
    body { background: #101418; color: #eef2f5; margin: 0; display: flex; min-height: 100vh; }
    nav { width: 200px; flex-shrink: 0; padding: 2rem 1rem; background: #171e25; }
    nav a { display: block; color: #b8c8d6; padding: .6rem; }
    main { max-width: 1100px; width: 100%; margin: auto; padding: 2rem; box-sizing: border-box; }
    article { background: #1b2229; border: 1px solid #303b45; border-radius: 10px; padding: 1rem; margin: 1rem 0; }
    button { background: #4f9cf9; border: 0; border-radius: 6px; color: white; cursor: pointer; padding: .55rem .8rem; }
    button + button { margin-left: .4rem; }
    input, select { background: #101418; color: #eef2f5; border: 1px solid #536474; border-radius: 4px; padding: .55rem; margin: .25rem; }
    label { display: inline-block; margin: .3rem; }
    button:disabled { cursor: wait; opacity: .6; }
    dl { display: grid; grid-template-columns: 180px 1fr; gap: .35rem 1rem; }
    dt { color: #9fb0bf; } dd { margin: 0; }
    pre { white-space: pre-wrap; color: #b8c8d6; }
    .healthy { color: #73d99a; } .unhealthy, .failed { color: #ff8f8f; }
    img { max-width: 100%; }
    @media (max-width: 700px) { body { display: block; } nav { width: auto; padding: .5rem; } nav a { display: inline-block; } main { padding: 1rem; } dl { grid-template-columns: 1fr; } }
  </style>
</head>
<body>
<nav aria-label="Administration">
  <strong>Sentinel Streaming</strong>
  <a href="#dashboard">Dashboard</a>
  <a href="#camera-setup">Cameras</a>
  <a href="#vision">AI Vision</a>
  <a href="#events">Events</a>
  <a href="#diagnostics">Diagnostics</a>
</nav>
<main>
  <h1>Sentinel Streaming Admin</h1>
  <p>RTSP source validation, stream health, and ONVIF capability inspection.</p>
  <p id="security-mode" class="healthy">Security mode: loading…</p>
  <section id="auth-panel">
    <h2>Sign in</h2>
    <p>Use the administrator/operator/viewer token configured for this installation. First-run deployments may use the explicitly configured bootstrap token.</p>
    <input id="auth-token" type="password" autocomplete="current-password" placeholder="Access token" aria-label="Access token">
    <button id="auth-login">Sign in</button>
    <button id="auth-logout">Sign out</button>
    <span id="auth-status"></span>
  </section>
  <section id="dashboard">
    <h2>Dashboard</h2>
    <p id="dashboard-status">Loading service status...</p>
    <dl id="dashboard-metrics"></dl>
    <button id="refresh-overview">Refresh overview</button>
  </section>
  <section id="camera-setup">
    <h2>Add camera</h2>
    <p>Find a camera, verify it, preview it, and save it without entering RTSP or ONVIF details.</p>
    <button id="discover">Find cameras</button>
    <div id="discovered"></div>
    <details>
      <summary>Advanced: add a source manually</summary>
      <form id="manual-source">
        <label>Source ID <input name="id" required pattern="[A-Za-z0-9_-]+" maxlength="64"></label>
        <label>Type <select name="kind"><option value="rtsp">RTSP</option><option value="built-in-camera">Built-in / USB</option><option value="synthetic">Synthetic</option></select></label>
        <label>RTSP URL (no credentials) <input name="uri" placeholder="rtsp://camera/stream"></label>
        <button type="submit">Add source</button>
        <p id="manual-source-status" role="status"></p>
      </form>
    </details>
  </section>
  <section id="cameras"><p>Loading sources…</p></section>
  <section id="vision">
    <h2>AI Vision</h2><p id="vision-status">Loading vision status...</p>
    <p>Changes apply to this process only and are lost on restart. The OpenAI key is configured outside the console.</p>
    <form id="vision-form">
      <label><input type="checkbox" name="enabled"> Enable vision</label>
      <label>OpenAI model <input name="model" required maxlength="128" pattern="[A-Za-z0-9_.:-]+"></label>
      <label>Interval (seconds) <input name="intervalSeconds" type="number" min="1" max="3600" required></label>
      <label>Frames <input name="frames" type="number" min="1" max="10" required></label>
      <label>Spacing (seconds) <input name="spacingSeconds" type="number" min="1" max="3600" required></label>
      <button type="submit">Apply vision settings</button><span id="vision-form-status" role="status"></span>
    </form>
    <pre id="vision-latest"></pre>
  </section>
  <section id="events"><h2>Recent events</h2><p>In-memory history is bounded and resets on restart. Severity is not currently recorded.</p>
    <label>Category <input id="event-category" placeholder="e.g. vision"></label>
    <label>Source <input id="event-source" placeholder="Source ID"></label>
    <div id="event-list">Loading events...</div>
  </section>
  <section id="diagnostics"><h2>Diagnostics</h2><p>Health and recent component failures are shown below. Support bundles do not include image data.</p><pre id="diagnostic-status">Loading diagnostics...</pre><button id="support-bundle">Download support bundle</button><p id="support-status" role="status"></p></section>
</main>
<script>
const rawFetch = window.fetch.bind(window);
let authToken = sessionStorage.getItem('sentinel_access_token') || '';
window.fetch = (input, init = {}) => {
  const headers = new Headers(init.headers || {});
  const url = new URL(typeof input === 'string' ? input : input.url, window.location.href);
  if (authToken && url.origin === window.location.origin) headers.set('Authorization', `Bearer ${authToken}`);
  return rawFetch(input, {...init, headers});
};
const authStatus = document.querySelector('#auth-status');
const securityMode = document.querySelector('#security-mode');
const authPanel = document.querySelector('#auth-panel');
document.querySelector('#auth-token').value = authToken;
document.querySelector('#auth-logout').addEventListener('click', () => {
  authToken = '';
  sessionStorage.removeItem('sentinel_access_token');
  document.querySelector('#auth-token').value = '';
  window.location.reload();
});
document.querySelector('#auth-login').addEventListener('click', async () => {
  const candidate = document.querySelector('#auth-token').value;
  authToken = candidate;
  const response = await fetch('/api/v1/auth/whoami');
  if (!response.ok) {
    authToken = '';
    sessionStorage.removeItem('sentinel_access_token');
    authStatus.textContent = 'Authentication failed. Check the token and try again.';
    return;
  }
  sessionStorage.setItem('sentinel_access_token', authToken);
  const identity = await response.json();
  authStatus.textContent = `Signed in as ${identity.principal?.id || 'authenticated user'}.`;
  await load();
  await loadOverview();
});
const cameras = document.querySelector('#cameras');
const discovered = document.querySelector('#discovered');
const previewCleanup = new Set();
function text(value) { return value == null ? '—' : String(value); }
function capabilitySummary(capabilities) {
  if (!capabilities) return 'Not inspected';
  const labels = ['video', 'audio', 'snapshot', 'events'];
  const supported = labels.filter(key => capabilities[key]).join(', ');
  return `${supported || 'No media capabilities'}; PTZ: ${capabilities.ptz && capabilities.ptz.supported ? 'supported' : 'not supported'}`;
}
function productError(result) {
  const message = document.createElement('p');
  message.className = 'unhealthy';
  message.textContent = result.message || result.error || 'Camera setup could not be completed.';
  wizard.append(message);
  if (result.details) {
    const details = document.createElement('details');
    const summary = document.createElement('summary');
    summary.textContent = 'Technical details';
    const pre = document.createElement('pre');
    pre.textContent = result.details;
    details.append(summary, pre);
    wizard.append(details);
  }
}
function renderChecks(checks) {
  const list = document.createElement('ul');
  (checks || []).forEach(check => {
    const item = document.createElement('li');
    item.textContent = `${check.id}: ${check.message}`;
    item.className = check.state === 'fail' ? 'unhealthy' : 'healthy';
    list.append(item);
  });
  return list;
}
function renderInspection(session) {
  wizard.replaceChildren();
  const heading = document.createElement('h3');
  heading.textContent = session.selected_device ? `${session.selected_device.manufacturer || 'Camera'} ${session.selected_device.model || ''}` : 'Camera inspection';
  const summary = document.createElement('p');
  const profile = session.selected_profile;
  summary.textContent = profile ? `Selected automatically: ${profile.name}${profile.resolution ? `, ${profile.resolution}` : ''}${profile.encoding ? `, ${profile.encoding}` : ''}` : 'No usable video profile was found.';
  wizard.append(heading, summary, renderChecks(session.checks));
  if (!profile) return;
  const name = document.createElement('input');
  name.placeholder = 'Camera name, e.g. Front Door';
  name.required = true;
  const location = document.createElement('input');
  location.placeholder = 'Location (optional)';
  const complete = document.createElement('button');
  complete.textContent = 'Save and finish setup';
  complete.addEventListener('click', async () => {
    complete.disabled = true;
    const response = await fetch(`/api/v1/onboarding/sessions/${encodeURIComponent(session.session_id)}/complete`, {
      method: 'POST', headers: {'content-type': 'application/json'},
      body: JSON.stringify({source_id: `camera-${Date.now()}`, name: name.value, location: location.value || null})
    });
    const result = await response.json();
    wizard.replaceChildren();
    if (!response.ok || !result.success) {
      productError(result.failure || result);
      wizard.append(renderChecks(result.checks));
    } else {
      const ready = document.createElement('p');
      ready.className = 'healthy';
      ready.textContent = 'Camera added successfully. Browser live preview is ready.';
      wizard.append(ready, renderChecks(result.checks));
      await load();
    }
    complete.disabled = false;
  });
  wizard.append(name, location, complete);
}
const wizard = document.querySelector('#discovered');
document.querySelector('#discover').addEventListener('click', async (event) => {
  const button = event.currentTarget;
  button.disabled = true;
  try {
    const response = await fetch('/api/v1/onboarding/discover', { method: 'POST', headers: {'content-type': 'application/json'}, body: '{}' });
    const result = await response.json();
    if (!response.ok) { productError(result); return; }
    discovered.replaceChildren(...(result.devices || []).map(device => {
      const card = document.createElement('article');
      const title = document.createElement('h3');
      title.textContent = `${device.manufacturer || 'ONVIF'} ${device.model || 'camera'}`;
      const details = document.createElement('pre');
      details.textContent = `Capabilities: ${capabilitySummary(device.capabilities)}\nProfiles: ${(device.profiles || []).map(profile => `${profile.name} ${profile.resolution || ''} ${profile.encoding || ''}`).join('; ') || 'inspect to determine available profiles'}`;
      const select = document.createElement('button');
      select.textContent = 'Select camera';
      select.addEventListener('click', () => {
        const username = document.createElement('input'); username.placeholder = 'Username (only if required)';
        const password = document.createElement('input'); password.type = 'password'; password.placeholder = 'Password (only if required)';
        const inspect = document.createElement('button'); inspect.textContent = 'Inspect capabilities';
        inspect.addEventListener('click', async () => {
          inspect.disabled = true;
          const inspection = await fetch(`/api/v1/onboarding/sessions/${encodeURIComponent(result.session_id)}/inspect`, {
            method: 'POST', headers: {'content-type': 'application/json'},
            body: JSON.stringify({endpoint: device.address, username: username.value || null, password: password.value || null})
          });
          const inspected = await inspection.json();
          if (!inspection.ok) productError(inspected); else renderInspection(inspected);
          inspect.disabled = false;
        });
        card.append(username, password, inspect);
      });
      card.append(title, details, select);
      return card;
    }));
    if (!(result.devices || []).length) discovered.textContent = 'No cameras were found. Try discovery again or use Advanced manual RTSP setup.';
  } finally { button.disabled = false; }
});
function render(sources) {
  for (const cleanup of previewCleanup) cleanup();
  previewCleanup.clear();
  if (!Array.isArray(sources)) {
    cameras.textContent = 'Camera inventory is unavailable. Sign in with an authorized account.';
    return;
  }
  if (!sources.length) {
    cameras.textContent = 'No cameras are registered.';
    return;
  }
  cameras.replaceChildren(...sources.map(source => {
    const card = document.createElement('article');
    const title = document.createElement('h2');
    title.textContent = source.name || source.id;
    const details = document.createElement('dl');
    [['ID', source.id], ['Type', source.type], ['Lifecycle', source.status],
     ['Health', source.health], ['Validation', source.validation],
     ['Recovery', source.recovery], ['Recovery attempts', source.recovery_attempts],
     ['Next recovery', source.next_recovery_at],
     ['Last attempt', source.last_validation_attempt],
     ['Last success', source.last_successful_validation],
     ['Last recovery success', source.last_recovery_succeeded],
     ['Last recovery exhausted', source.last_recovery_exhausted],
     ['Media state', source.media_telemetry && source.media_telemetry.deliveryState],
     ['Protocol', source.media_telemetry && source.media_telemetry.protocol],
     ['Codec', source.media_telemetry && source.media_telemetry.codec],
     ['Resolution', source.media_telemetry && source.media_telemetry.resolution],
     ['Observed FPS', source.media_telemetry && source.media_telemetry.observedFps],
     ['Bitrate', source.media_telemetry && source.media_telemetry.bitrateBps ? `${Math.round(source.media_telemetry.bitrateBps / 1000)} kbps` : null],
     ['Audio', source.media_telemetry && source.media_telemetry.audioPresent === true ? 'Available' : source.media_telemetry && source.media_telemetry.audioPresent === false ? 'Not available' : null],
     ['Audio codec', source.media_telemetry && source.media_telemetry.audioCodec],
     ['Audio sample rate', source.media_telemetry && source.media_telemetry.audioSampleRate ? `${source.media_telemetry.audioSampleRate} Hz` : null],
     ['Audio channels', source.media_telemetry && source.media_telemetry.audioChannels],
     ['Audio transport', source.media_telemetry && source.media_telemetry.audioDeliveryState],
     ['Audio activity', source.media_telemetry && source.media_telemetry.lastAudioActivity ? new Date(Number(source.media_telemetry.lastAudioActivity)).toLocaleTimeString() : null],
     ['Last activity', source.media_telemetry && source.media_telemetry.lastMediaActivity ? new Date(Number(source.media_telemetry.lastMediaActivity)).toLocaleTimeString() : null],
     ['Playback', source.media_telemetry && (source.media_telemetry.playbackProtocols || []).join(', ')],
     ['Capabilities', source.capabilities ? `Video: ${source.capabilities.video ? 'Yes' : 'No'}; Audio: ${source.capabilities.audio ? 'Yes' : 'No'}; PTZ: ${source.capabilities.ptz && source.capabilities.ptz.supported ? 'Supported' : 'Not supported'}` : 'Not inspected']].forEach(([label, value]) => {
      const dt = document.createElement('dt'); dt.textContent = label;
      const dd = document.createElement('dd'); dd.textContent = text(value);
      if (label === 'Health') dd.className = String(value || '').toLowerCase();
      details.append(dt, dd);
    });
    const button = document.createElement('button');
    button.textContent = source.type === 'rtsp' ? 'Validate RTSP' : 'RTSP only';
    button.disabled = source.type !== 'rtsp';
    button.addEventListener('click', async () => {
      button.disabled = true;
      try {
        const response = await fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/validate`, { method: 'POST' });
        const result = await response.json();
        const diagnostic = document.createElement('pre');
        diagnostic.textContent = JSON.stringify(result, null, 2);
        card.append(diagnostic);
        await load();
      } finally { button.disabled = false; }
    });
    card.append(title, details, button);
    const operationStatus = document.createElement('p');
    operationStatus.setAttribute('role', 'status');
    for (const [label, operation, method] of [['Start', 'start', 'POST'], ['Stop', 'stop', 'POST'], ['Remove', '', 'DELETE']]) {
      const action = document.createElement('button');
      action.textContent = label;
      action.addEventListener('click', async () => {
        if (label === 'Remove' && !confirm(`Remove ${source.id}?`)) return;
        action.disabled = true;
        try {
          const suffix = operation ? `/${operation}` : '';
          const response = await fetch(`/api/v1/sources/${encodeURIComponent(source.id)}${suffix}`, {method});
          if (!response.ok) {
            const result = await response.json();
            operationStatus.textContent = result.error || `${label} failed (${response.status}).`;
            return;
          }
          await load();
          await loadOverview();
        } catch (error) {
          operationStatus.textContent = `${label} failed: ${error.message}`;
        } finally { action.disabled = false; }
      });
      card.append(action);
    }
    const preview = document.createElement('button');
    preview.textContent = 'Start live preview';
    const image = document.createElement('img');
    image.alt = `Current frame from ${source.name || source.id}`;
    image.hidden = true;
    let active = false;
    let timer;
    function stopPreview() {
      active = false;
      clearTimeout(timer);
      if (image.src.startsWith('blob:')) URL.revokeObjectURL(image.src);
      image.hidden = true;
      preview.textContent = 'Start live preview';
    }
    previewCleanup.add(stopPreview);
    async function updatePreview() {
      if (!active) return;
      try {
        const response = await fetch(`/api/v1/streams/${encodeURIComponent(source.id)}/frame`);
        if (!response.ok) throw new Error(`Frame unavailable (${response.status})`);
        const blob = await response.blob();
        if (!active) return;
        const old = image.src;
        image.src = URL.createObjectURL(blob);
        image.hidden = false;
        if (old.startsWith('blob:')) URL.revokeObjectURL(old);
        timer = setTimeout(updatePreview, 1000);
      } catch (error) { operationStatus.textContent = error.message; stopPreview(); }
    }
    preview.addEventListener('click', () => {
      if (active) { stopPreview(); return; }
      active = true;
      preview.textContent = 'Stop live preview';
      updatePreview();
    });
    card.append(preview, operationStatus, image);
    if (source.media_telemetry && source.media_telemetry.detail) {
      const mediaMessage = document.createElement('p');
      mediaMessage.className = source.media_telemetry.deliveryState === 'READY' ? 'healthy' : 'unhealthy';
      mediaMessage.textContent = source.media_telemetry.detail;
      card.append(mediaMessage);
    }
    if (source.type === 'rtsp') {
      const live = document.createElement('section');
      const liveTitle = document.createElement('h3');
      liveTitle.textContent = 'Live view';
      const status = document.createElement('p');
      status.textContent = `Media gateway: ${text(source.media_health)}`;
      const video = document.createElement('video');
      video.controls = true; video.autoplay = true; video.muted = true; video.playsInline = true;
      video.style.width = '100%'; video.style.background = '#050608';
      const play = document.createElement('button');
      play.textContent = 'Publish and open live view';
      play.addEventListener('click', async () => {
        play.disabled = true;
        try {
          const registered = await fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/playback/register`, {method: 'POST'});
          const playback = await registered.json();
          if (!registered.ok) { status.textContent = playback.error || 'Media gateway unavailable'; return; }
          status.textContent = `Media gateway: ${playback.media_health}`;
          const webrtc = (playback.streams || []).find(stream => stream.protocol === 'webrtc');
          const hls = (playback.streams || []).find(stream => stream.protocol === 'hls');
          if (webrtc && window.RTCPeerConnection) {
          const peer = new RTCPeerConnection();
          peer.ontrack = event => { if (event.streams[0]) video.srcObject = event.streams[0]; };
          peer.addTransceiver('video', {direction: 'recvonly'});
            const audio = source.media_telemetry && source.media_telemetry.audioPresent === true;
            if (audio) peer.addTransceiver('audio', {direction: 'recvonly'});
            video.muted = true;
            video.autoplay = true;
            video.playsInline = true;
            const offer = await peer.createOffer();
            await peer.setLocalDescription(offer);
            const answer = await fetch(webrtc.url, {method: 'POST', headers: {'content-type': 'application/sdp'}, body: offer.sdp});
            if (answer.ok) {
              await peer.setRemoteDescription({type: 'answer', sdp: await answer.text()});
            } else if (hls) { video.src = hls.url; }
          } else if (hls) { video.src = hls.url; }
        } finally { play.disabled = false; }
      });
      const audioToggle = document.createElement('button');
      audioToggle.textContent = 'Unmute audio';
      audioToggle.disabled = !(source.media_telemetry && source.media_telemetry.audioPresent === true);
      audioToggle.addEventListener('click', () => {
        video.muted = !video.muted;
        audioToggle.textContent = video.muted ? 'Unmute audio' : 'Mute audio';
        if (!video.muted) video.play().catch(() => {});
      });
      live.append(liveTitle, status, play, audioToggle, video);
      card.append(live);
      const capture = document.createElement('section');
      const captureTitle = document.createElement('h3');
      captureTitle.textContent = 'Capture';
      const snapshot = document.createElement('button');
      snapshot.textContent = 'Capture snapshot';
      const clip = document.createElement('button');
      clip.textContent = 'Capture 10s clip';
      const captureStatus = document.createElement('p');
      async function captureArtifact(path, body) {
        const response = await fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/${path}`, {
          method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify(body || {})
        });
        const result = await response.json();
        if (!response.ok) { captureStatus.textContent = result.error || 'Capture failed.'; captureStatus.className = 'unhealthy'; return; }
        captureStatus.className = 'healthy';
        captureStatus.textContent = `${result.artifactType || 'Artifact'} captured.`;
        const link = document.createElement('a');
        link.href = `/api/v1/media-artifacts/${encodeURIComponent(result.artifactId)}/content`;
        link.textContent = result.artifactType === 'CLIP' ? 'Download clip' : 'Download snapshot';
        link.target = '_blank';
        captureStatus.append(' ', link);
      }
      snapshot.addEventListener('click', () => captureArtifact('snapshots'));
      clip.addEventListener('click', () => captureArtifact('clips', {duration_seconds: 10}));
      capture.append(captureTitle, snapshot, clip, captureStatus);
      card.append(capture);
    }
    const ptz = source.capabilities && source.capabilities.ptz;
    if (ptz && ptz.supported) {
      const controls = document.createElement('section');
      const heading = document.createElement('h3');
      heading.textContent = 'PTZ test controls';
      controls.append(heading);
      const move = async (pan, tilt, zoom, mode = 'continuous') => {
        const response = await fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/ptz/move`, {
          method: 'POST', headers: {'content-type': 'application/json'},
          body: JSON.stringify({mode, pan, tilt, zoom})
        });
        const result = await response.json();
        const diagnostic = document.createElement('pre');
        diagnostic.textContent = JSON.stringify(result, null, 2);
        controls.append(diagnostic);
      };
      [['Up', 0, 0.5], ['Left', -0.5, 0], ['Stop', 0, 0], ['Right', 0.5, 0], ['Down', 0, -0.5]].forEach(([label, pan, tilt]) => {
        const control = document.createElement('button');
        control.textContent = label;
        control.disabled = label !== 'Stop' && ((pan !== 0 && !ptz.pan) || (tilt !== 0 && !ptz.tilt) || !ptz.continuous_move);
        control.addEventListener('click', () => label === 'Stop'
          ? fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/ptz/stop`, {method: 'POST'})
          : move(pan, tilt, 0));
        controls.append(control);
      });
      ['+', '-'].forEach(sign => {
        const control = document.createElement('button');
        control.textContent = `Zoom ${sign}`;
        control.disabled = !ptz.zoom || !ptz.continuous_move;
        control.addEventListener('click', () => move(0, 0, sign === '+' ? 0.5 : -0.5));
        controls.append(control);
      });
      if (ptz.presets) {
        const presets = document.createElement('button');
        presets.textContent = 'Load presets';
        presets.addEventListener('click', async () => {
          const response = await fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/ptz/presets`);
          const result = await response.json();
          (result.presets || []).forEach(preset => {
            const goto = document.createElement('button');
            goto.textContent = `Go to ${preset.name}`;
            goto.addEventListener('click', () => fetch(`/api/v1/sources/${encodeURIComponent(source.id)}/ptz/presets/${encodeURIComponent(preset.id)}/goto`, {method: 'POST'}));
            controls.append(goto);
          });
        });
        controls.append(presets);
      }
      card.append(controls);
    }
    return card;
  }));
}
async function load() {
  const response = await fetch('/api/v1/sources');
  if (!response.ok) throw new Error(`Camera inventory unavailable (${response.status})`);
  render(await response.json());
}
document.querySelector('#manual-source').addEventListener('submit', async event => {
  event.preventDefault();
  const form = event.currentTarget;
  const status = document.querySelector('#manual-source-status');
  const values = new FormData(form);
  const data = {id: values.get('id'), kind: values.get('kind')};
  if (data.kind === 'rtsp') data.uri = values.get('uri');
  try {
    const response = await fetch('/api/v1/sources', {method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify(data)});
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `Request failed (${response.status})`);
    status.textContent = 'Source registered for this runtime. Start it from the camera inventory.';
    form.reset();
    await load();
  } catch (error) { status.textContent = error.message; }
});
async function loadOverview() {
  const status = document.querySelector('#dashboard-status');
  try {
    const response = await fetch('/api/v1/status');
    if (!response.ok) throw new Error(`Status unavailable (${response.status}); sign in with diagnostic access.`);
    const result = await response.json();
    status.textContent = `Service version ${result.version}; ${result.securityMode}.`;
    const sourcesResponse = await fetch('/api/v1/sources');
    const sources = sourcesResponse.ok ? await sourcesResponse.json() : null;
    const activeSources = Array.isArray(sources) ? sources.filter(source => source.status === 'running').length : 'Unavailable';
    const list = document.querySelector('#dashboard-metrics');
    list.replaceChildren();
    for (const [label, value] of [
      ['Uptime', `${result.metrics.uptime_seconds} s`], ['Running sources', activeSources],
      ['Frames per second', result.metrics.fps.toFixed(1)], ['Dropped frames', result.metrics.dropped_frames],
      ['Memory', `${result.metrics.memory_bytes} bytes`], ['CPU', `${result.metrics.cpu_percent.toFixed(1)}%`],
      ['Buffer', `${result.buffer.length} / ${result.buffer.capacity}`],
      ['Active connections', 'Not measured'], ['Stream latency', 'Not measured']
    ]) {
      const dt = document.createElement('dt'); dt.textContent = label;
      const dd = document.createElement('dd'); dd.textContent = value;
      list.append(dt, dd);
    }
    const vision = result.vision;
    document.querySelector('#vision-status').textContent =
      `Effective state: ${vision.enabled ? (vision.running ? 'enabled' : 'enabled, not running') : 'disabled'}; credential ${vision.credentialConfigured ? 'configured' : 'not configured'}; model ${vision.model}; interval ${vision.intervalSeconds}s; frames ${vision.frames}; spacing ${vision.spacingSeconds}s. Analyses: ${vision.metrics.requests}; failures: ${vision.metrics.failures}; HTTP attempts: ${vision.metrics.http_attempts}; rate limits: ${vision.metrics.rate_limits}; average latency: ${vision.metrics.average_latency_ms == null ? 'unavailable' : vision.metrics.average_latency_ms + ' ms'}. Input/output tokens: ${vision.metrics.input_tokens == null ? 'unavailable' : vision.metrics.input_tokens + ' / ' + vision.metrics.output_tokens}.`;
    const form = document.querySelector('#vision-form');
    form.elements.enabled.checked = vision.enabled;
    for (const name of ['model', 'intervalSeconds', 'frames', 'spacingSeconds']) form.elements[name].value = vision[name];
    document.querySelector('#diagnostic-status').textContent = JSON.stringify(result.health, null, 2);
    const latestResponse = await fetch('/api/v1/vision/latest');
    document.querySelector('#vision-latest').textContent = latestResponse.ok ? JSON.stringify(await latestResponse.json(), null, 2) : 'No observation available.';
    const eventsResponse = await fetch('/api/v1/events');
    if (!eventsResponse.ok) throw new Error(`Events unavailable (${eventsResponse.status})`);
    recentEvents = await eventsResponse.json();
    renderEvents();
  } catch (error) { status.textContent = error.message; }
}
let recentEvents = [];
function renderEvents() {
  const list = document.querySelector('#event-list');
  const category = document.querySelector('#event-category').value.trim().toLowerCase();
  const source = document.querySelector('#event-source').value.trim().toLowerCase();
  const filtered = recentEvents.filter(item => item.event_type.toLowerCase().includes(category) && (item.source_id || '').toLowerCase().includes(source));
  list.replaceChildren();
  if (!filtered.length) list.textContent = 'No matching events.';
  for (const item of filtered) {
    const entry = document.createElement('p');
    entry.textContent = `${new Date(Number(item.timestamp)).toLocaleString()} — ${item.event_type} — ${item.source_id || 'system'} — ${item.summary}`;
    list.append(entry);
  }
}
document.querySelector('#event-category').addEventListener('input', renderEvents);
document.querySelector('#event-source').addEventListener('input', renderEvents);
document.querySelector('#refresh-overview').addEventListener('click', loadOverview);
document.querySelector('#vision-form').addEventListener('submit', async event => {
  event.preventDefault();
  const form = event.currentTarget;
  const status = document.querySelector('#vision-form-status');
  const settings = {enabled: form.elements.enabled.checked, model: form.elements.model.value,
    intervalSeconds: Number(form.elements.intervalSeconds.value), frames: Number(form.elements.frames.value),
    spacingSeconds: Number(form.elements.spacingSeconds.value)};
  try {
    const response = await fetch('/api/v1/vision/config', {method: 'PUT', headers: {'content-type': 'application/json'}, body: JSON.stringify(settings)});
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `Request failed (${response.status})`);
    status.textContent = 'Applied for this process; settings revert after restart.';
    await loadOverview();
  } catch (error) { status.textContent = error.message; }
});
document.querySelector('#support-bundle').addEventListener('click', async () => {
  const status = document.querySelector('#support-status');
  try {
    const response = await fetch('/api/v1/support/bundle');
    if (!response.ok) throw new Error(`Support bundle unavailable (${response.status})`);
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url; link.download = 'sentinel-support.json'; link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    status.textContent = 'Support bundle downloaded; review it before sharing.';
  } catch (error) { status.textContent = error.message; }
});
async function loadSecurityMode() {
  const response = await rawFetch('/api/v1/version');
  if (!response.ok) return;
  const version = await response.json();
  securityMode.textContent = `Security mode: ${version.securityMode || 'UNKNOWN'}`;
  if (version.securityMode === 'OPEN_LOCAL_TEST') {
    securityMode.textContent += ' — development/test mode; authentication disabled';
    authPanel.hidden = true;
  }
}
loadSecurityMode().catch(() => { securityMode.textContent = 'Security mode: unavailable'; });
load().catch(error => { cameras.textContent = `Unable to load sources: ${error.message}`; });
loadOverview();
</script>
</body>
</html>"##;

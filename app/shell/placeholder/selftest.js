// Android spike: checks that bulk data crosses the Rust <-> UI boundary as
// raw bytes, both ways, and that the render buffer decodes to the values the
// desktop build produces for the same program on the generic mill.
"use strict";

const PROGRAM = "G21 G90 G54 G17\nG0 X0. Y0. Z5.\nG1 Z-1. F100.\nG2 X10. Y0. R5.\nM30\n";

async function selfTest() {
  const out = document.getElementById("out");
  const lines = [];
  let failures = 0;
  const check = (name, ok, detail) => {
    lines.push(`${ok ? "PASS" : "FAIL"} ${name}${detail === undefined ? "" : ": " + detail}`);
    if (!ok) failures += 1;
  };
  const near = (a, b) => Math.abs(a - b) < 1e-4;
  const tauri = window.__TAURI_INTERNALS__;
  try {
    check("Tauri IPC present", !!(tauri && tauri.invoke));
    const invoke = tauri.invoke.bind(tauri);
    lines.push(`platform: ${navigator.userAgent}`);

    const dialects = await invoke("list_dialects");
    check("list_dialects (small JSON)", dialects.length === 2 && dialects[0].id === "fanuc", dialects.map((d) => d.id).join(", "));

    const name = await invoke("select_machine", { source: { kind: "shipped", id: "generic_3axis_mill" } });
    check("select_machine", name === "Generic 3-axis VMC", name);

    // Program as the raw request body; render buffer back as raw bytes.
    const t0 = performance.now();
    const buf = await invoke("interpret", new TextEncoder().encode(PROGRAM), { headers: { "x-file-name": "selftest.nc" } });
    const ms = (performance.now() - t0).toFixed(1);
    const kind = Object.prototype.toString.call(buf);
    check("interpret returns raw bytes (ArrayBuffer)", buf instanceof ArrayBuffer, `${kind}, ${ms} ms`);
    const v = new DataView(buf);
    check("buffer length", buf.byteLength === 564, buf.byteLength);
    check("magic", v.getUint32(0, true) === 0x47435052, "0x" + v.getUint32(0, true).toString(16));
    check("version", v.getUint16(4, true) === 1, v.getUint16(4, true));
    const counts = [8, 12, 16, 20].map((o) => v.getUint32(o, true));
    check("vertex/segment/diag/channel counts", counts.join(",") === "31,4,0,1", counts.join(","));
    const origin = [24, 32, 40].map((o) => v.getFloat64(o, true));
    check("origin (f64)", near(origin[0], -190) && near(origin[1], -102.5) && near(origin[2], -200.5), origin.join(", "));
    const vtx = v.getUint32(48, true);
    // Zero-copy typed-array view over the positions (section is 4-byte aligned).
    const pos = new Float32Array(buf, vtx, 31 * 3);
    const last = [pos[90], pos[91], pos[92]];
    check("last vertex (f32 view)", near(last[0], -180) && near(last[1], -102.5) && near(last[2], -200.5), last.join(", "));
    const seg = v.getUint32(52, true);
    const kinds = [0, 1, 2, 3].map((i) => v.getUint8(seg + i * 24 + 18));
    const srcLines = [0, 1, 2, 3].map((i) => v.getUint32(seg + i * 24 + 8, true));
    check("segment kinds", kinds.join(",") === "0,1,2,15", kinds.join(","));
    check("segment lines", srcLines.join(",") === "2,3,4,5", srcLines.join(","));

    // A larger program: 20 000 blocks, to see the timing on the device.
    let big = "G21 G90 G54 G17\nG0 X0. Y0. Z5.\nG1 F500.\n";
    for (let i = 0; i < 20000; i += 1) big += `X${(i % 100) * 3}. Y${Math.floor(i / 100) % 100}.\n`;
    const t1 = performance.now();
    const bigBuf = await invoke("interpret", new TextEncoder().encode(big), {});
    const bigMs = (performance.now() - t1).toFixed(0);
    const bigSegs = new DataView(bigBuf).getUint32(12, true);
    check("20 000-block program", bigSegs === 20001, `${bigSegs} segments, ${bigBuf.byteLength} bytes, ${bigMs} ms`);

    // File round trip: raw bytes both ways, non-ASCII path in the header.
    const folder = await invoke("machine_folder");
    const path = `${folder}/selftest-ü.nc`;
    const bytes = new Uint8Array(256);
    for (let i = 0; i < 256; i += 1) bytes[i] = i;
    await invoke("save_file", bytes, { headers: { "x-path": encodeURIComponent(path) } });
    const loaded = new Uint8Array(await invoke("load_file", { path }));
    const same = loaded.length === 256 && loaded.every((b, i) => b === i);
    check("save_file then load_file (all 256 byte values)", same, `${loaded.length} bytes, ${path}`);
  } catch (e) {
    check("no error thrown", false, String(e));
  }
  lines.unshift(failures === 0 ? "ALL PASSED" : `${failures} FAILED`);
  out.textContent = lines.join("\n");
  out.className = failures === 0 ? "pass" : "fail";
  try {
    await tauri.invoke("report", { text: lines.join("\n") });
  } catch (_) {
    // Reporting to the log is only a convenience.
  }
}

document.getElementById("run").addEventListener("click", () => {
  document.getElementById("out").textContent = "Running...";
  selfTest();
});
selfTest();

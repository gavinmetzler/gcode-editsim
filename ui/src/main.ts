// The UI entry point. M2.4 is the scaffold: the toolpath viewport (M2.5),
// editor (M2.6) and panels (M2.7) come next.
import { VERSION } from "./generated/render_buffer";

const app = document.getElementById("app");
if (app) {
  app.innerHTML = `
    <h1>GCode EditSim</h1>
    <p>User interface under construction (render buffer layout v${String(VERSION)}).</p>
    <p class="disclaimer">
      This software visualizes and analyzes G-code. It is not a safety
      verification tool. Always verify programs independently before running
      them on a machine.
    </p>`;
}

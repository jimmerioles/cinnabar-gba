// Capture hook for this project.
//
// `explainer capture` opens `capture.url` in Chromium with the viewport from
// config, takes a still of `capture.selector`, and saves it to
// docs/explainer/assets/app.png. Edit this file when a project needs
// interaction: clicking a key, waiting for audio, or recording a take.
//
// Delete this file to use the built-in capture (url plus selector) instead.

export default async function capture({ page, config, out }) {
  await page.goto(config.capture.url, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(600);
  await page.locator(config.capture.selector ?? "body").screenshot({
    path: `${out}/app.png`,
  });
  return ["app.png"];
}

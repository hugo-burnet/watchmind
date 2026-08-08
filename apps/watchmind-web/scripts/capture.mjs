import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ deviceScaleFactor: 1 });

await page.setViewportSize({ width: 1440, height: 1050 });
await page.goto("http://127.0.0.1:4173", { waitUntil: "networkidle" });
await page.screenshot({ path: "../../docs/screenshots/watchmind-l15-desktop.png", fullPage: true });

await page.setViewportSize({ width: 390, height: 844 });
await page.reload({ waitUntil: "networkidle" });
await page.screenshot({ path: "../../docs/screenshots/watchmind-l15-mobile.png", fullPage: true });

await browser.close();

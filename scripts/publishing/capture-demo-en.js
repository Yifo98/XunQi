export default async function captureDemoEn(page) {
  const projectRoot = process.cwd();
  const root = process.env.XUNQI_CAPTURE_DIR ?? `${projectRoot}/output/playwright`;
  const shot = async (index) => {
    await page.waitForTimeout(350);
    await page.screenshot({ path: `${root}/en-${String(index).padStart(2, "0")}.png` });
  };

  await page.reload();
  await page.waitForLoadState("networkidle");
  await page.setViewportSize({ width: 1920, height: 1080 });
  await page.addScriptTag({ path: `${projectRoot}/scripts/publishing/translate-demo-en.js` });
  await page.waitForTimeout(500);
  await shot(1);

  await page.getByRole("textbox", { name: "Search tasks" }).fill("https://mp.weixin.qq.com/s/public-demo-link");
  await page.waitForTimeout(600);
  await shot(2);

  await page.locator("button").filter({ hasText: "Test Article: Local Content Workflow" }).first().click();
  await page.getByRole("button", { name: "Collapse capture queue" }).click();
  await shot(3);

  await page.getByRole("button", { name: "Expand capture queue" }).click();
  await page.locator("button").filter({ hasText: "Test Article: Automatic Intake and Classification" }).first().click();
  await page.getByRole("combobox").selectOption("markdown");
  await shot(4);

  await page.getByRole("combobox").selectOption("pdf");
  await page.getByRole("button", { name: "Export original-layout PDF" }).click();
  await page.waitForTimeout(450);
  await shot(5);

  await page.locator("button").filter({ hasText: "Test Video: Public Direct-Link Detection" }).first().click();
  await shot(6);

  await page.getByRole("button", { name: "Show more (3)" }).click();
  await page.locator("button").filter({ hasText: "Test Video: No Public Direct Link" }).first().click();
  await page.getByRole("button", { name: "Authorize detection download" }).click();
  await page.waitForTimeout(400);
  await shot(7);

  await page.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: "About" }).click();
  await page.waitForTimeout(300);
  await shot(8);
}

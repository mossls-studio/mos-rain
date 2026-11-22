// 真引擎冒烟测试：扫描 mos_rain helper → 建 960×540 合成 → 测试程序雨滴与自定义亮度/Alpha 图层提取 → 出 4 张图。
// 运行：先在同进程扫描 helper，再跑本脚本（见 README 或以下命令）：
//   effectcraft-cli run effect.helpers.scan "{}" script.run "{\"code\":\"...\"}"
var OUT = "D:\\mossls\\mos\\形变\\mos_液态\\mos_雨滴\\tests\\out\\engine";
writeLn("helpers -> " + JSON.stringify(app.run("effect.helpers.list", {})));

// 1. 背景板合成（色块 + 格子 + 大字，便于观察折射与色散）
var bgComp = app.project.items.addComp("rain_source", 960, 540, 1, 1 / 30, 30);
bgComp.layers.addSolid([0.08, 0.16, 0.24], "bg", 960, 540, 1);
var card = bgComp.layers.addSolid([0.85, 0.44, 0.16], "card", 380, 240, 1);
card
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([300, 220]);
var bar = bgComp.layers.addSolid([0.16, 0.72, 0.86], "bar", 420, 130, 1);
bar
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([660, 380]);
var title = bgComp.layers.addText("MOS RAIN");
title
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([480, 310]);
var td = title.property("ADBE Text Properties").property("ADBE Text Document");
var doc = td.value;
doc.fontSize = 104;
doc.fillColor = [1, 0.96, 0.88];
td.setValue(doc);

// 2. 自定义亮度贴图合成（黑底 + 白字 + 亮色块，与合成同尺寸以确保 layer_input 位置严格对齐）
var lumaComp = app.project.items.addComp("luma_map", 960, 540, 1, 1 / 30, 30);
lumaComp.layers.addSolid([0.0, 0.0, 0.0], "black_bg", 960, 540, 1);
var lumaBlock = lumaComp.layers.addSolid([0.82, 0.82, 0.82], "luma_block", 210, 150, 1);
lumaBlock
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([230, 175]);
var lumaTxt = lumaComp.layers.addText("WATER");
lumaTxt
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([560, 330]);
var ltd = lumaTxt.property("ADBE Text Properties").property("ADBE Text Document");
var ldoc = ltd.value;
ldoc.fontSize = 112;
ldoc.fillColor = [1, 1, 1];
ltd.setValue(ldoc);

// 3. 自定义 Alpha 贴图合成（透明底 + 纯白图形文字轮廓）
var alphaComp = app.project.items.addComp("alpha_map", 960, 540, 1, 1 / 30, 30);
var alphaTxt = alphaComp.layers.addText("DROP");
alphaTxt
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([480, 290]);
var atd = alphaTxt.property("ADBE Text Properties").property("ADBE Text Document");
var adoc = atd.value;
adoc.fontSize = 148;
adoc.fillColor = [1, 1, 1];
atd.setValue(adoc);

// 4. 主测试合成
var c = app.project.items.addComp("rain_test", 960, 540, 1, 1 / 30, 30);
c.openInViewer();

var lumaLayer = c.layers.add(lumaComp);
lumaLayer.enabled = false;
var alphaLayer = c.layers.add(alphaComp);
alphaLayer.enabled = false;
var plate = c.layers.add(bgComp);

var res = app.run("effect.apply", { layer: plate.id, effect: "mos.rain" });
writeLn("apply -> " + JSON.stringify(res && res.params ? res.params : res));

var shots = [
  ["engine-01-procedural", 0, { custom_layer: -1, bg_blur: 18 }],
  ["engine-02-custom-luma", 0, { custom_layer: lumaLayer.id, custom_mode: 0, custom_channel: 0, custom_threshold: 18, custom_bevel: 16, custom_disp: 110, bg_blur: 0, dispersion: 36 }],
  ["engine-03-custom-alpha", 0, { custom_layer: alphaLayer.id, custom_mode: 0, custom_channel: 1, custom_threshold: 18, custom_bevel: 14, custom_disp: 120, bg_blur: 0, dispersion: 55 }],
  ["engine-04-custom-blend", 0, { custom_layer: alphaLayer.id, custom_mode: 1, custom_channel: 1, custom_bevel: 14, bg_blur: 18, dispersion: 35 }],
];

for (var i = 0; i < shots.length; i++) {
  var shot = shots[i];
  for (var k in shot[2]) {
    app.run("prop.set", {
      layer: plate.id,
      path: "effects/#1/" + k,
      value: shot[2][k],
    });
  }
  app.run("time.set", { time: shot[1] });
  var t0 = new Date().getTime();
  var r = app.run("render.saveCurrentPreview", {
    comp: c.id,
    path: OUT + "\\" + shot[0] + ".png",
  });
  writeLn(shot[0] + " -> " + (new Date().getTime() - t0) + "ms  " + JSON.stringify(r));
}
"done";

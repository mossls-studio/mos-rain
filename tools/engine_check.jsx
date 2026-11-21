// 真引擎冒烟测试：加载 mos_rain.wasm → 建 960×540 合成 → 应用 mos.rain → 出 3 张图。
// 运行（本仓库不自动跑，由维护者统一执行）：
//   effectcraft-cli script tools/engine_check.jsx
var PLUGIN = "D:\\mossls\\mos\\形变\\mos_液态\\mos_雨滴\\dist\\zh-zh\\mos_rain.wasm";
var OUT = "D:\\mossls\\mos\\形变\\mos_液态\\mos_雨滴\\tests\\out\\engine";
app.run("effect.plugins.load", { path: PLUGIN });

// 内层：背景 + 色块 + 文字，给折射一点可看的内容。
var inner = app.project.items.addComp("rain_source", 960, 540, 1, 5, 30);
inner.layers.addSolid([0.05, 0.11, 0.25], "bg", 960, 540, 1);
var card = inner.layers.addSolid([0.85, 0.42, 0.12], "card", 380, 240, 1);
card
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([300, 220]);
var title = inner.layers.addText("MOS RAIN");
title
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([480, 330]);
var td = title.property("ADBE Text Properties").property("ADBE Text Document");
var doc = td.value;
doc.fontSize = 96;
doc.fillColor = [1, 0.95, 0.85];
td.setValue(doc);

// 外层：把内层当成一整张画面，效果才能同时作用到背景和文字上。
var c = app.project.items.addComp("rain_test", 960, 540, 1, 5, 30);
var plate = c.layers.add(inner);
plate
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([480, 270]);

var res = app.run("effect.apply", { layer: plate.id, effect: "mos.rain" });
writeLn("apply -> " + JSON.stringify(res && res.params ? res.params : res));

var shots = [
  ["rain-default", 0, {}],
  ["rain-streak", 2, { streak: 100, trail: 65, speed: 45 }],
  ["rain-mist", 4, { size: 8, density: 0, mist: 100, streak: 0, amount: 100, highlight: 140 }],
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

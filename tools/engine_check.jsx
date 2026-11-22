// 真引擎冒烟测试：加载 mos_rain.wasm → 建 960×540 合成 → 应用 mos.rain → 出 3 张图。
// 运行：
//   effectcraft-cli script tools/engine_check.jsx
var PLUGIN = "D:\\mossls\\mos\\形变\\mos_液态\\mos_雨滴\\dist\\zh-zh\\mos_rain.wasm";
var OUT = "D:\\mossls\\mos\\形变\\mos_液态\\mos_雨滴\\tests\\out\\engine";
app.run("effect.plugins.load", { path: PLUGIN });

var inner = app.project.items.addComp("rain_source", 960, 540, 1, 1 / 30, 30);
inner.layers.addSolid([0.08, 0.16, 0.24], "bg", 960, 540, 1);
var card = inner.layers.addSolid([0.82, 0.45, 0.18], "card", 380, 240, 1);
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

var c = app.project.items.addComp("rain_test", 960, 540, 1, 1 / 30, 30);
c.openInViewer();
var plate = c.layers.add(inner);
plate
  .property("ADBE Transform Group")
  .property("ADBE Position")
  .setValue([480, 270]);

var res = app.run("effect.apply", { layer: plate.id, effect: "mos.rain" });
writeLn("apply -> " + JSON.stringify(res && res.params ? res.params : res));

var shots = [
  ["rain-default", 0, {}],
  ["rain-streaks", 0.5, { big_density: 25, small_density: 40, streak_density: 85, bg_blur: 28 }],
  ["rain-dispersion", 0, { dispersion: 75, displacement: 140, big_size: 75 }],
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

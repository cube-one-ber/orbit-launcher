import QtQuick

Canvas {
    id: icon
    property string name: "grid"
    property color ink: "white"
    implicitWidth: 20; implicitHeight: 20
    Accessible.ignored: true
    onNameChanged: requestPaint()
    onInkChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onPaint: {
        const c = getContext("2d"); c.reset(); c.scale(width / 24, height / 24);
        c.strokeStyle = ink; c.fillStyle = ink; c.lineWidth = 1.65; c.lineCap = "round"; c.lineJoin = "round";
        const line = (x,y,x2,y2) => { c.beginPath(); c.moveTo(x,y); c.lineTo(x2,y2); c.stroke(); };
        const circle = (x,y,r) => { c.beginPath(); c.arc(x,y,r,0,Math.PI*2); c.stroke(); };
        if (name === "grid") { for (let x of [4,14]) for (let y of [4,14]) c.strokeRect(x,y,6,6); }
        else if (name === "list") { for (let y of [6,12,18]) { line(4,y,5,y); line(10,y,20,y); } }
        else if (name === "search") { circle(10,10,6); line(15,15,21,21); }
        else if (name === "plus") { line(12,5,12,19); line(5,12,19,12); }
        else if (name === "close") { line(6,6,18,18); line(18,6,6,18); }
        else if (name === "clock") { circle(12,12,8); line(12,7,12,12); line(12,12,16,14); }
        else if (name === "play") { c.beginPath(); c.moveTo(8,5); c.lineTo(19,12); c.lineTo(8,19); c.closePath(); c.fill(); }
        else if (name === "star") { c.beginPath(); for(let i=0;i<10;i++){ const a=i*Math.PI/5-Math.PI/2, r=i%2 ? 4:9; const x=12+Math.cos(a)*r,y=12+Math.sin(a)*r; if(i===0)c.moveTo(x,y);else c.lineTo(x,y); } c.closePath(); c.stroke(); }
        else if (name === "moon") { c.beginPath(); c.arc(12,12,8,0.1,4.6); c.quadraticCurveTo(7,15,20,13); c.stroke(); }
        else if (name === "sun") { circle(12,12,4); for(let i=0;i<8;i++){ let a=i*Math.PI/4; line(12+Math.cos(a)*7,12+Math.sin(a)*7,12+Math.cos(a)*10,12+Math.sin(a)*10); } }
        else if (name === "refresh") { c.beginPath(); c.arc(12,12,8,0.4,5.5); c.stroke(); line(18,3,18,8); line(18,8,13,8); }
        else if (name === "settings") { for(let y of [6,12,18]) line(3,y,21,y); for(let p of [[8,6],[16,12],[10,18]]){c.fillRect(p[0]-2,p[1]-2,4,4);} }
        else if (name === "folder") { c.beginPath(); c.moveTo(3,6); c.lineTo(10,6); c.lineTo(12,9); c.lineTo(21,9); c.lineTo(21,20); c.lineTo(3,20); c.closePath(); c.stroke(); }
        else if (name === "chevron") { line(8,9,12,13); line(12,13,16,9); }
        else { c.strokeRect(4,4,16,16); line(4,9,20,9); line(9,9,9,20); }
    }
}

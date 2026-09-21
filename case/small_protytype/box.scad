// v5 CLOSED : 60 x 30 x 20, front hole centered on new wall height, boss auto-lengthened
W=60; D=30; H=20; T=1.5; CLR=0.2;
LID_TOP=2.5; BOX_H=H-LID_TOP; LID_X1=W/2-CLR; LID_PX0=LID_X1-T; LID_Y=D/2-T-CLR; LID_Z0=T+CLR;
HOLE_W=52; HOLE_H=5.2; HOLE_ZC=H/2;
BH_W=10; BH_H=3.5; BH_GAP=1.5;
PAD_X=10; PAD_Z=0.5; PAD_FRAC=2.0/3.0; PAD_Y=(D-2*T)*PAD_FRAC; PAD_YB=D/2-T;
SINK=1.0; CUT_BACK=1.0;
BOSS_R=4.0; INS_R=2.0; INS_DEPTH=6.0;
SCREW_R=1.6; CSK_R=3.0; CSK_H=CSK_R-SCREW_R; BOSS_X=LID_PX0-BOSS_R-0.3; BOSS_Y=0;
EXPLODE=0; $fn=64;
module box_body(){ difference(){ union(){
  translate([-W/2,-D/2,0]) cube([W,D,T]);
  translate([-W/2,-D/2,T]) cube([W,T,H-T]);
  translate([-W/2,D/2-T,T]) cube([W,T,H-T]);
  translate([-PAD_X/2,PAD_YB-PAD_Y,T-SINK]) cube([PAD_X,PAD_Y,PAD_Z+SINK]);
  for (sx=[1,-1]) translate([sx*BOSS_X,BOSS_Y,T-SINK]) cylinder(r=BOSS_R,h=BOX_H-T+SINK);
 } translate([-HOLE_W/2,-D/2-CUT_BACK,HOLE_ZC-HOLE_H/2]) cube([HOLE_W,T+2*CUT_BACK,HOLE_H]);
   translate([-BH_W/2,D/2-T-CUT_BACK,T+BH_GAP]) cube([BH_W,T+2*CUT_BACK,BH_H]);
 }
}
// 열삽입 구멍은 보스 상단(뚜껑 밑면)에서 아래로
module ins(){ for (sx=[1,-1]) translate([sx*BOSS_X,BOSS_Y,BOX_H-INS_DEPTH]) cylinder(r=INS_R,h=INS_DEPTH+1); }
module box_final(){ difference(){ box_body(); ins(); } }
module lid(){ difference(){ union(){
  translate([-LID_X1,-LID_Y,BOX_H]) cube([2*LID_X1,2*LID_Y,LID_TOP]);
  translate([LID_PX0,-LID_Y,LID_Z0]) cube([T,2*LID_Y,H-LID_Z0]);
  translate([-LID_X1,-LID_Y,LID_Z0]) cube([T,2*LID_Y,H-LID_Z0]);
 } for (sx=[1,-1]) translate([sx*BOSS_X,BOSS_Y,0]) {
     cylinder(r=SCREW_R,h=H+1);
     translate([0,0,H-CSK_H]) cylinder(r1=SCREW_R,r2=CSK_R,h=CSK_H);
     translate([0,0,H]) cylinder(r=CSK_R,h=1); } } }
color([0.55,0.78,0.95]) box_final();
translate([0,0,EXPLODE]) color([1.00,0.72,0.72]) lid();

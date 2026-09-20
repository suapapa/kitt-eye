L    = 33;
W    = 23;
H    = 15;
ANG   = 10;
Z0    = 50;
T_SK  = 2;
FUSE  = 0.6;
Y_SPLIT    = 5.0;
Y_OUT_UP   = 11.5;
BOSS_R     = 3.0; // 4
BOSS_TOP_Y = 8.0;
BOSS_BOT_Y = -8.5;
INS_D      = 3; // 4.1
INS_DEPTH  = 9.0;
INS_CHAM   = 0.1; //0.6;
POCK_D     = (BOSS_R+0.2)*2; // 8.4;
POCK_TOP_Y = 8.4;
POCK_BOT_Y = 4.75;
SCREW_D    = 3.4;
Y_SCREW_BOT = 2.5;
Y_SCREW_TOP = 12.5;
CSK_D      = 6.6;
CSK_H      = (CSK_D - SCREW_D) / 2;
BOSS_INSET     = 2; // 1.2;
BOSS_END_INSET = BOSS_R + 1.0;
module light() {
    TX = 1; TY = 2; T = 1; DRAFT = 14; HOLE = 10;
    d = (H - T) * tan(DRAFT);
    e = 1 * tan(DRAFT);
    module cavity() { hull() {
        translate([TX + d, TY + d, T]) cube([L - 2*TX - 2*d, W - 2*TY - 2*d, 0.001]);
        translate([TX - e, TY - e, H + 1]) cube([L - 2*TX + 2*e, W - 2*TY + 2*e, 0.001]); } }
    module floor_hole() { translate([(L - HOLE)/2, (W - HOLE)/2, -T/2]) cube([HOLE, HOLE, 2*T]); }
    translate([-L/2, 0, -H]) difference() { cube([L, W, H]); cavity(); floor_hole(); }
}
module fan() {
    for (i = [0:1:3]) {
        rotate([0, ANG, 0])  translate([L/2 + L*i, 0, 0])  light();
        rotate([0, -ANG, 0]) translate([-(L/2 + L*i), 0, 0]) light();
    }
    rotate([0, ANG, 0])  translate([L*4 - 1, 0, -H]) cube([2, W, H]);
    rotate([0, -ANG, 0]) translate([-(L*4 + 1), 0, -H]) cube([2, W, H]);
}
module object() { translate([0, 0, Z0]) fan(); }
XE  = (L*4 + 1) * cos(ANG);
XB  = (L*4 + 1) * cos(ANG) - H * sin(ANG);
T_C = Z0 - H / cos(ANG);
S_A = Z0 - (L*4 + 1) * sin(ANG);
TA  = tan(ANG);
function TV(x) = T_C - TA * abs(x);
function TS(x) = S_A - (XE - abs(x)) / TA;
function TT(x) = Z0 - TA * abs(x);
function TE(x) = min(TS(x) + FUSE, TT(x));
XC_P = ((L*4+1) * sin(ANG) - FUSE + XE / TA) / (1 / TA + TA);
ZE  = TT(XE);
DT  = TS(XC_P) + FUSE;
ZB  = TV(XB) + FUSE;
ZC  = TV(0)  + FUSE;
ARCH_MID = (ZC + Z0) / 2 - 4.5;
BOSS_XI  = [ for (i = [1 : 2 : 1]) i * L * cos(ANG) - BOSS_INSET ];
BOSS_XE  = XE - BOSS_END_INSET-1;
BOSSES   = concat(
               [ for (x = BOSS_XI) [ x, ARCH_MID - TA * x ] ],
               [ [ BOSS_XE, ARCH_MID - TA * BOSS_XE - 10 ] ]
           );
               
profile_full = [
    [-XE, 0], [ XE, 0],
    [ XE, ZE], [ XC_P, DT], [ XB, ZB], [ 0, ZC], [-XB, ZB], [-XC_P, DT], [-XE, ZE]
];
profile_end_r = [[XE - T_SK, 0], [XE, 0], [XE, ZE], [XC_P, DT], [XE - T_SK, TE(XE - T_SK)]];
profile_end_l = [[-(XE - T_SK), 0], [-XE, 0], [-XE, ZE], [-XC_P, DT], [-(XE - T_SK), TE(XE - T_SK)]];
module wall(pts, y0, hh) {
    translate([0, y0 + hh, 0]) rotate([90, 0, 0]) linear_extrude(height = hh) polygon(pts);
}
module body() {
    translate([0, -W/2, 0]) {
        union() {
            wall(profile_full,  0,        T_SK);
            wall(profile_full,  W - T_SK, T_SK);
            wall(profile_end_r, 0,        W);
            wall(profile_end_l, 0,        W);
            object();
        }
    }
}
module boss_list() {
    for (p = BOSSES) for (s = [1, -1])
        translate([s * p[0], (BOSS_BOT_Y + BOSS_TOP_Y) / 2, p[1]])
            rotate([-90, 0, 0]) cylinder(r = BOSS_R, h = BOSS_TOP_Y - BOSS_BOT_Y, center = true, $fn = 64);
}
module insert_holes() {
    hh = INS_DEPTH - INS_CHAM;
    for (p = BOSSES) for (s = [1, -1])
        translate([s * p[0], 0, p[1]]) {
            translate([0, BOSS_TOP_Y - INS_CHAM - hh / 2, 0])
                rotate([-90, 0, 0]) cylinder(r = INS_D / 2, h = hh, center = true, $fn = 48);
            translate([0, BOSS_TOP_Y - INS_CHAM / 2, 0])
                rotate([-90, 0, 0]) cylinder(r1 = INS_D/2, r2 = INS_D/2 + INS_CHAM, h = INS_CHAM+1, center = true, $fn = 48);
        }
}
module boss_relief() {
    for (p = BOSSES) for (s = [1, -1])
        translate([s * p[0], (POCK_BOT_Y + POCK_TOP_Y) / 2, p[1]])
            rotate([-90, 0, 0]) cylinder(r = POCK_D / 2, h = POCK_TOP_Y - POCK_BOT_Y, center = true, $fn = 64);
}
module screw_holes() {
    for (p = BOSSES) for (s = [1, -1])
        translate([s * p[0], 0, p[1]]) {
            translate([0, (Y_SCREW_BOT + Y_SCREW_TOP) / 2, 0]) rotate([-90, 0, 0])
                cylinder(r = SCREW_D / 2, h = Y_SCREW_TOP - Y_SCREW_BOT, center = true, $fn = 48);
            translate([0, Y_OUT_UP - CSK_H / 2, 0]) rotate([-90, 0, 0])
                cylinder(r1 = SCREW_D / 2, r2 = SCREW_D / 2 + CSK_H, h = CSK_H, center = true, $fn = 48);
        }
}
module halfspace_low()  { translate([-200, -200, -100]) cube([400, 200 + Y_SPLIT, 300]); }
module halfspace_high() { translate([-200, Y_SPLIT, -100]) cube([400, 300, 300]); }
module lower_part() {
    difference() {
        union() {
            intersection() { body(); halfspace_low(); }
            boss_list();
        }
        insert_holes();
    }

    // back-plate
    module usb_c(){
        rotate([0,0,0]) union() {
            cube([13.5,5.5,20], center=true);
            cube([14,3.5,20], center=true);        
        }
    }
    
    difference() {
        translate([-XE,-W/2,-2]) cube([XE*2,W,2]);
        translate([0,0,-H/2]) usb_c();
        for(i=[-20,20]) {
            translate([i,W/2-8/2-2,8/2]) 
                cylinder(h=20, r=3.2/2, $fn=20, center=true);
        }
    }
}
module upper_part() {
    difference() {
        intersection() { body(); halfspace_high(); }
        boss_relief();
        screw_holes();
        
        // holes for stand
        translate([50/2,0,15]) rotate([90,0,0]) cylinder(h=50, r=3.2/2, $fn=20, center=true);
        translate([-50/2,0,15]) rotate([90,0,0]) cylinder(h=50, r=3.2/2, $fn=20, center=true);
        translate([0,0,15]) rotate([90,0,0]) cylinder(h=50, r=3.2/2, $fn=20, center=true);
    }
    
    for(i=[-20,20]) {
        translate([i,W/2-8/2-2,8/2]) difference(){
            cube(8, center=true);
            translate([0,0,-1]) rotate([0,0,0]) cylinder(h=9, r=4/2, $fn=20, center=true);
    }}
}
module lower_print() { translate([0, 0, W/2]) rotate([ 90, 0, 0]) lower_part(); }
module upper_print() { translate([0, 0, W/2]) rotate([-90, 0, 0]) upper_part(); }


PART = 1;
if      (PART == 0) lower_print();
else if (PART == 1) upper_print();
else if (PART == 2) upper_part();    
else if (PART == 3) lower_part();    
else if (PART == 4) { lower_part(); upper_part(); }
else                { lower_print(); translate([0, 10, 0]) upper_print(); }

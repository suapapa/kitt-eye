d=30;
t=24;
b=27;
h=23;
z=-(h-4);
y=2;
w=100;

difference(){
    union() {
        translate([-w/2, -d/2, -10]) difference(){
            cube([w, d, 10]);
            //translate([2, -5, 12]) cube([w-2*2, d+10, 5]);
            translate([-2, 0, 0])
                rotate([90, 0 ,90])
                    linear_extrude(height=w+2*2)
                        polygon([[y, z], [y, z+h], [y+t, z+h], [y+b, z]]);
        }
        //translate([50/2,0,0]) cylinder(h=5, r=2.9/2, center=true, $fn=20);
        //translate([-50/2,0,0]) cylinder(h=5, r=2.9/2, center=true, $fn=20);
    }
    cylinder(h=10, r=4/2, center=true, $fn=20);
    translate([50/2,0,0]) cylinder(h=10, r=4/2, center=true, $fn=20);
    translate([-50/2,0,0]) cylinder(h=10, r=4/2, center=true, $fn=20);
}

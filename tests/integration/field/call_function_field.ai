class Foo {}

fn bar(a, b) {
  print "bar";
  print a;
  print b;
}

let foo = Foo();
foo.bar = bar;

foo.bar(1, 2);
// expect: bar
// expect: 1
// expect: 2

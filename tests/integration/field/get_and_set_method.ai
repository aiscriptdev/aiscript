// Bound methods have identity equality.
class Foo {
  method(a) {
    print "method";
    print a;
  }
  other(a) {
    print "other";
    print a;
  }
}

let foo = Foo();
let method = foo.method;

// Setting a property shadows the instance method.
foo.method = foo.other;
foo.method(1);
// expect: other
// expect: 1

// The old method handle still points to the original method.
method(2);
// expect: method
// expect: 2

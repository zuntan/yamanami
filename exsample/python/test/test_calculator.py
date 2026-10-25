import sys
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from calculator import calculate

class Arithmetic(unittest.TestCase):
    def test_operations(self):
        for a, op, b, expected in [("2","add","3",5),("7","sub","9",-2),
                                    ("-2.5","mul","4",-10),("9","div","4",2.25)]:
            self.assertEqual(calculate(a,op,b), expected)
    def test_errors(self):
        for a, op, b in [("1","div","0"),("1","div","-0"),("NaN","add","1"),
                          ("1e309","add","1"),("","add","2"),("1x","add","2"),
                          ("1","eval","2"),("1e308","mul","1e308")]:
            with self.subTest(a=a,op=op,b=b), self.assertRaises(ValueError):
                calculate(a,op,b)

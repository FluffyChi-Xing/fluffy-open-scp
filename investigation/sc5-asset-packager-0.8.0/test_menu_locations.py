import unittest,sys,struct,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).parent/'engine'))
from menu_locations import MENU_LOCATIONS,menu_record,destination
from build_candidate import array,prop_bytes
from inspect_park_route import parse
from package_io import package_bytes,index
class Entry:
    def __init__(self,data):self.data=data
    def read(self):return self.data
class MenuRoutingTests(unittest.TestCase):
    def test_all_routes_preserve_exact_memberships_through_package(self):
        for category,d in MENU_LOCATIONS.items():
            with self.subTest(category=category),tempfile.TemporaryDirectory() as temp:
                donor=array(0x0db9fc63,32,[struct.pack('>3I',d['categoryInstance'],0,0)],12)
                record,evidence=menu_record({d['donor']:Entry(prop_bytes({0x0db9fc63:donor}))},category)
                menu={0x0db9fc63:record,0x0dc1e3e0:array(0x0dc1e3e0,9,[struct.pack('>i',100)],4)}
                p=Path(temp)/'route.package';p.write_bytes(package_bytes({(0xb1b104,0x09878a01,0x11223344):prop_bytes(menu)}))
                readback=parse(index(p)[0].read())
                self.assertEqual(readback['0DB9FC63']['value'],evidence['exportedCategoryKeys'])
                self.assertEqual(len(readback['0DB9FC63']['value']),len(readback['0DC1E3E0']['value']))
    def test_wrong_native_category_and_missing_donor_are_rejected(self):
        donor=MENU_LOCATIONS['health']['donor']
        wrong=array(0x0db9fc63,32,[struct.pack('>3I',123,0,0)],12)
        with self.assertRaisesRegex(ValueError,'不一致'):menu_record({donor:Entry(prop_bytes({0x0db9fc63:wrong}))},'health')
        with self.assertRaisesRegex(ValueError,'缺少'):menu_record({},'health')
        with self.assertRaisesRegex(ValueError,'不支持'):destination('commercial')
    def test_education_keeps_legacy_multiple_memberships(self):
        d=MENU_LOCATIONS['education'];keys=[[0,0,0x6a1785b4],[0,0,d['categoryInstance']]]
        donor=array(0x0db9fc63,32,[struct.pack('>3I',k[2],k[0],k[1]) for k in keys],12)
        record,evidence=menu_record({d['donor']:Entry(prop_bytes({0x0db9fc63:donor}))},'education')
        self.assertEqual(parse(prop_bytes({0x0db9fc63:record}))['0DB9FC63']['value'],keys)
if __name__=='__main__':unittest.main()

"""Native recipe integration tests: corruption rejection and lifecycle isolation."""
import json
from pathlib import Path
import unittest
import behavior_profiles as b

class Entry:
    def __init__(self,data):self.data=data
    def read(self):return self.data

class BehaviorProfiles(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        b.engine_imports()
        cls.native,_=b.native_index('D:/E/gameecg/SimCity：Cites of Tomorrow')
        from build_candidate import cloned
        cls.baseline=cloned((Path(__file__).parent/'engine/tested-unit.bin').read_bytes())

    def settings(self,key=b.PARK):return {'behaviorProfile':key,'cost':12345,'maintenancePerHour':67}

    def test_native_recipe_preserves_custom_identity_and_footprint(self):
        from build_candidate import prop_bytes,parse
        out,e=b.apply(self.baseline,self.native,self.settings())
        for h in b.PROTECTED:
            self.assertEqual(out.get(h),self.baseline.get(h))
        for h in b.OMIT:self.assertNotIn(h,out)
        parsed=parse(prop_bytes(out),True);b.verify(parsed,e)
        self.assertEqual(parsed['0AFE27B1']['value'],[0,0,2308696688])
        self.assertEqual(parsed['0E965A70']['value'],1.)
        self.assertEqual(parsed['0E965A71']['value'],[2.5,2.5])
        self.assertEqual(parsed['0A8A41DA']['value'],12345.)
        self.assertEqual(parsed['09AE19D7']['value'],67.)
        self.assertEqual(e['candidateRuleCount'],205)
        self.assertEqual(len(e['validatedResources']),46)
        self.assertFalse(e['runtimeVerified'])
        # Menu selection is intentionally absent from the recipe interface.
        self.assertEqual(b.apply(self.baseline,self.native,self.settings())[0],out)

    def test_legacy_is_byte_identical_except_previous_fee_and_classification_overrides(self):
        from build_candidate import floatprop,key,prop_bytes,parse
        expected=dict(self.baseline)
        expected[0x0A8A41DA]=floatprop(0x0A8A41DA,12345)
        expected[0x09AE19D7]=floatprop(0x09AE19D7,67)
        expected[0x0AFE27B1]=key(0x0AFE27B1,(0,0,0x95A9D6FD))
        out,e=b.apply(self.baseline,{},self.settings(b.LEGACY))
        self.assertEqual(out,expected);b.verify(parse(prop_bytes(out),True),e)
        self.assertEqual(b.selection({}),b.LEGACY)

    def test_missing_or_changed_rule_source_rejected(self):
        pin=b._pin();k=b._key('08068AEB:40800200:622B9CD7')
        native=dict(self.native);native.pop(k)
        with self.assertRaisesRegex(ValueError,'依赖缺失'):b.resolve(native)
        native[k]=Entry(b'invalid-rule-source')
        with self.assertRaisesRegex(ValueError,'不一致'):b.resolve(native)

    def test_corrupt_emitted_effect_rejected(self):
        from build_candidate import prop_bytes,parse,floatprop
        out,e=b.apply(self.baseline,self.native,self.settings())
        out[0x0E965A70]=floatprop(0x0E965A70,0)
        with self.assertRaisesRegex(ValueError,'回读不一致'):b.verify(parse(prop_bytes(out),True),e)

    def test_unsupported_services_and_guessed_parameters_rejected(self):
        for s in ({'behaviorProfile':'school'},{'provideSimulationService':True},
                  {'behaviorProfile':b.PARK,'behaviorParameters':{'radius':100}},
                  {'provideSimulationService':1},{'behaviorProfile':[]},
                  {'behaviorProfile':b.PARK,'behaviorParameters':[]}):
            with self.subTest(s=s),self.assertRaises(ValueError):b.selection(s)

if __name__=='__main__':unittest.main()

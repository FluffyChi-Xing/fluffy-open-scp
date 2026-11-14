import unittest
import numpy as np
from asset_transforms import rotate_xyz,rotate_lot,room_strip


class OrientationNightTests(unittest.TestCase):
    def test_quarter_turn_geometry_normals_and_lot(self):
        points=np.array([[3.,-4.,2.],[-1.,6.,0.]])
        np.testing.assert_array_equal(rotate_xyz(points,180),points*[-1,-1,1])
        np.testing.assert_array_equal(rotate_xyz(points,90),[[4,3,2],[-6,-1,0]])
        normals=np.array([[1.,0.,0.],[0.,1.,0.]])
        rotated=rotate_xyz(normals,90)
        np.testing.assert_array_equal(np.linalg.norm(rotated,axis=1),[1,1])
        a=np.zeros((4,4,4),np.uint8);a[3,3,2]=255
        mask,size,offset,period=rotate_lot(a,[20,10],[3,-4],[8,4],90)
        self.assertEqual(size,[10,20]);self.assertEqual(offset,[4,3]);self.assertEqual(period,[4,8])
        # Source lower-right corner (+X,-Y) turns to upper-right (+X,+Y).
        self.assertEqual(mask[0,3,2],255)
        for angle in (0,90,180,270):
            np.testing.assert_array_equal(rotate_xyz(rotate_xyz(points,angle),(360-angle)%360),points)
        for bad in (True,45,-90,360):
            with self.assertRaises(ValueError):rotate_xyz(points,bad)

    def test_all_variations_no_day_emission_night_rgb_preserved(self):
        rng=np.random.default_rng(123)
        room=rng.integers(0,256,(512,512,4),dtype=np.uint8)
        before,_=room_strip(room,.125,.25,night_only=False)
        after,evidence=room_strip(room,.125,.25,night_only=True)
        np.testing.assert_array_equal(before[:,:,:3],after[:,:,:3])
        for state in range(16):
            tile=after[:,state*64:(state+1)*64]
            if state%4==1:np.testing.assert_array_equal(tile,before[:,state*64:(state+1)*64])
            else:self.assertFalse(tile[:,:,3].any())
        self.assertTrue(all(x['rgbUnchanged'] for x in evidence))
        # Already-dark source windows must remain dark at night as well.
        room[:,:,3]=0
        self.assertFalse(room_strip(room,.125,.25)[0][:,:,3].any())


if __name__=='__main__':unittest.main()

"""Conservative writer profile matching verified retail SimCity Property records.

An editor-readable representation is not automatically an approved game wire
profile. Numeric Lot fields are decoded and regenerated, never spliced verbatim.
This validator enforces our known-working profile, not every possible game type.
"""
import math,struct
from rw4_bridge import require
from inspect_park_route import parse

LOT_TYPES={0x0ccb7fc8:48,0x0ccb7fc9:48,0x0ccb7fd0:48,0x0ccb7fd6:9,
 **{0x0d02d586+i:52 for i in range(4)},0x0d02d58a:51,0x0d02d58b:51,
 **{0x0d7af042+i:52 for i in range(4)},**{0x0d7af046+i:13 for i in range(4)}}

def encode_lot_numeric(hash_id,field):
 require(hash_id in LOT_TYPES and field['type']==LOT_TYPES[hash_id],'Unsupported Lot field or numeric type')
 require(not field['flags']&0x30,'Lot numeric field must be scalar')
 typ=field['type'];value=field['value']
 if typ==9:
  require(isinstance(value,int) and -(2**31)<=value<2**31,'Lot integer overflow');payload=struct.pack('>i',value)
 else:
  values=[value] if typ==13 else value;count={13:1,48:2,51:4,52:4}[typ]
  require(len(values)==count and all(math.isfinite(x) for x in values),'Invalid Lot float vector')
  payload=struct.pack('>'+'f'*count,*values)
 return struct.pack('>IHH',hash_id,typ,0x8000)+payload

def validate_native_profile(data):
 fields=parse(data,True)
 for h,v in fields.items():
  require(v['flags']&0x8000,f'Property {h}: outside verified game-native wire profile (missing 0x8000); convert editor records before packaging')
 return fields

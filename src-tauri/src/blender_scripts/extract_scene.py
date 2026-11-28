"""Read scene attachments independently; never loads or executes the addon."""
import bpy
import json
import sys

class StoredGroup(bpy.types.PropertyGroup):
    pass

def plain(value):
    if hasattr(value, 'items'):
        return {key: plain(item) for key, item in value.items()}
    if hasattr(value, 'to_list'):
        return value.to_list()
    return value

def main():
    source, output = sys.argv[sys.argv.index('--') + 1:]
    bpy.utils.register_class(StoredGroup)
    bpy.types.Light.ny_native_settings = bpy.props.PointerProperty(type=StoredGroup)
    bpy.types.Object.ny_effect_settings = bpy.props.PointerProperty(type=StoredGroup)
    bpy.types.Object.ny_lot_regions = bpy.props.CollectionProperty(type=StoredGroup)
    bpy.ops.wm.open_mainfile(filepath=source)
    scene = bpy.context.scene
    result = {'objects': [], 'errors': []}
    for obj in bpy.context.view_layer.objects:
        if not obj.visible_get():
            continue
        kind = ('light' if obj.get('ny_native_light') else
                'effect' if obj.get('ny_native_effect') else
                'prop' if obj.get('ny_prop_preset') else
                'lot' if obj.get('ny_lot') else None)
        if not kind:
            continue
        try:
            row = {'kind': kind, 'name': obj.name, 'props': plain(obj),
                   'matrix': [float(x) for r in obj.matrix_world for x in r]}
            if kind == 'light':
                row['settings'] = plain(obj.data.ny_native_settings)
                row['lightType'] = obj.data.type
            elif kind == 'effect':
                row['settings'] = plain(obj.ny_effect_settings)
            elif kind == 'prop':
                row['meshUnit'] = float(obj.data.get('ny_prop_mesh_unit', 1.0))
            elif kind == 'lot':
                # RNA omits values equal to defaults. Defaults follow the
                # supplied 1.23.7 lot_paint.py NYLotRegion definition.
                defaults = {'color': [.3, .3, .3], 'native_index': 0,
                            'tile': .8, 'border': .15,
                            'border_color': [.16, .15, .13],
                            'height': 0., 'border_height': .5, 'border_index': 0}
                row['regions'] = [defaults | plain(r) for r in obj.ny_lot_regions]
                row['vertices'] = [list(v.co) for v in obj.data.vertices]
                row['resolution'] = int(obj.data.get('ny_lot_resolution', 0))
                row['mask'] = list(obj.data.get('ny_lot_mask', []))
            result['objects'].append(row)
        except Exception as error:
            result['errors'].append({'object': obj.name, 'detail': str(error)})
    with open(output, 'w', encoding='utf8') as handle:
        json.dump(result, handle, ensure_ascii=False)

if __name__ == '__main__':
    main()

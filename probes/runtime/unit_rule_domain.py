"""Typed H&V command subset: corpus signatures plus verified binary/runtime fields.

These are research contracts, not a recovered complete Eco command registry.
Unknown options/forms fail explicitly; all token order remains in the syntax node.
"""
import math
import re

from hero_villain_source_audit import fnv1

IDENTIFIER = r'[A-Za-z_][A-Za-z0-9_:]*'
VARIABLE = r'\$(?:\{' + IDENTIFIER + r'\}|' + IDENTIFIER + r')'
SCOPES = {'local', 'agent', 'nearby', 'connected', 'map', 'global'}


class ContractError(ValueError):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def require(value, message, code='UR020'):
    if not value:
        raise ContractError(code, message)


def symbol(kind, name):
    require(re.fullmatch(IDENTIFIER, name), 'expected symbol identifier')
    return dict(kind=kind, name=name, id=f'{fnv1(name):08X}')


def number(text, integer=False, nonnegative=False):
    if re.fullmatch(VARIABLE, text):
        return dict(kind='externalVariable', text=text)
    pattern = r'[+-]?\d+' if integer else r'[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?'
    require(re.fullmatch(pattern, text), 'unsupported numeric expression')
    try:
        value = int(text) if integer else float(text)
    except (ValueError, OverflowError) as exc:
        raise ContractError('UR020', 'numeric literal outside supported range') from exc
    require(not integer or -2147483648 <= value <= 2147483647, 'integer outside signed 32-bit subset')
    require(math.isfinite(value), 'non-finite numeric literal')
    require(not nonnegative or value >= 0, 'negative value outside supported subset')
    return dict(kind='integer' if integer else 'number', value=value, text=text)


def checked_options(view, signatures, repeated=()):
    result, seen = [], set()
    for option in view['options']:
        name = option['name'].lower()
        require(name in signatures, 'unsupported option: ' + option['name'], 'UR021')
        require(name not in seen or name in repeated, 'repeated singleton option: ' + name, 'UR022')
        seen.add(name)
        values = [t['value'] for t in option['arguments']]
        lo, hi = signatures[name]
        require(lo <= len(values) <= hi, 'wrong option argument count: ' + name, 'UR023')
        result.append((name, values, option))
    return result


def transport_options(view):
    signatures = {n: (1, 1) for n in ['sendto', 'resendto', 'or', 'createagent', 'using', 'via', 'dieafter', 'region']}
    signatures.update(switchto=(2, 2))
    signatures.update({n: (0, 0) for n in ['perapplication', 'resend', 'ignorestability']})
    result = []
    for name, values, original in checked_options(view, signatures, repeated={'or', 'switchto'}):
        item = dict(operation=name)
        if name in {'sendto', 'resendto', 'or', 'switchto'}:
            item['destination'] = symbol('sink', values[0])
            if name == 'switchto':
                item['life'] = number(values[1], nonnegative=True)
        elif name in {'createagent', 'via', 'using'}:
            item['reference'] = symbol('handler' if name == 'using' else 'transport', values[0])
        elif name == 'dieafter':
            item['life'] = number(values[0], nonnegative=True)
        elif name == 'region':
            item['regionRaw'] = values[0]
        item['sourceSpan'] = original['token']['span']
        result.append(item)
    return dict(kind='transportOptions', operations=result)


def lower(node):
    command = node['command']
    view = node['syntax']['arguments']
    args = [t['value'] for t in view['main']][1:]
    if command in {'applycount', 'maxapplycount'}:
        require(not view['options'], 'application count does not accept options in this subset')
        if command == 'maxapplycount':
            require(not args, 'maxApplyCount dynamic forms are not recovered')
            return dict(kind='applicationCount', mode='resourceBounded', rawBounds=[0, 0])
        require(len(args) in [1, 2], 'applyCount requires one or two bounds')
        values = [number(a, integer=True, nonnegative=True) for a in args]
        if all(v['kind'] == 'integer' for v in values):
            require(values[0]['value'] <= values[-1]['value'], 'reversed applyCount range outside supported subset')
        return dict(kind='applicationCount', mode='explicitBounds', minimum=values[0], maximum=values[-1])
    if command in {'flags', 'globalflags'}:
        require(not args, 'flag names must follow a supported flag option')
        ops = []
        for name, values, _ in checked_options(view, {'if': (1, 64), 'set': (1, 64), 'clear': (1, 64)}):
            ops.append(dict(operation='test' if name == 'if' else name,
                            symbols=[symbol('controlFlag', v) for v in values]))
        require(ops, 'empty control flag command')
        return dict(kind='controlFlags', scope='global' if command == 'globalflags' else 'local', operations=ops)
    if command == 'timetrigger':
        require(len(args) == 2, 'timeTrigger requires period and time')
        result = dict(kind='timeTrigger', period=symbol('period', args[0]), time=number(args[1]),
                      modifiers=[])
        for name, values, _ in checked_options(view, {'fromcreate': (0, 0), 'count': (1, 1), 'variance': (1, 1)}):
            result['modifiers'].append(dict(operation=name, value=True if name == 'fromcreate'
                                           else number(values[0], integer=name == 'count', nonnegative=True)))
        return result
    if command in {'rate', 'priority'}:
        require(len(args) == 1 and not view['options'], 'expected one numeric argument')
        return dict(kind=command, value=number(args[0], integer=True, nonnegative=command == 'rate'))
    if command == 'options':
        require(not args, 'options requires named options')
        require(view['options'], 'empty options command')
        return transport_options(view)
    if command in {'onsuccess', 'onfail', 'chain'}:
        actions = []
        require(not view['options'] or command == 'onsuccess', 'creation action branch outside observed contract', 'UR024')
        for name, values, original in checked_options(view, {'createunit': (2, 2)}):
            token = original['arguments'][1]
            require(token['kind'] == 'group', 'createUnit offset must be parenthesized')
            components = [v.strip() for v in values[1].split(',')]
            require(len(components) == 3, 'createUnit offset requires three comma-separated components')
            actions.append(dict(kind='createUnit', unit=symbol('unit', values[0]),
                                offset=[number(v) for v in components]))
        return dict(kind='ruleEdge', branch=command, target=symbol('unitRule', args[0]) if args else None, actions=actions)
    if command == 'testagent':
        require(len(args) == 1, 'testAgent requires one handler')
        options = checked_options(view, {'canreach': (1, 1)})
        require(len(options) == 1, 'testAgent subset requires canReach')
        return dict(kind='agentTest', handler=symbol('handler', args[0]),
                    canReach=symbol('sink', options[0][1][0]))
    if command == 'successevent':
        require(len(args) >= 2, 'successEvent requires category and target')
        category = args[0].lower()
        require(category in {'uievent', 'telemetry', 'vignette', 'alert', 'history'}, 'unsupported event category', 'UR024')
        options = checked_options(view, {'resource': (2, 2), 'start': (0, 0), 'stop': (0, 0), 'clear': (0, 0)})
        require(len({n for n, _, _ in options if n in {'start', 'stop', 'clear'}}) <= 1, 'conflicting event actions')
        allowed = {'uievent': set(), 'telemetry': {'resource'}, 'vignette': {'start', 'stop'},
                   'alert': {'clear'}, 'history': set()}[category]
        require(all(n in allowed for n, _, _ in options), 'event option outside observed category contract', 'UR024')
        result = dict(kind='successEvent', category=category, target=symbol('eventSymbol', args[1]), modifiers=[])
        if category == 'history':
            require(len(args) == 4 and args[2].lower() in {'local', 'global'}, 'history requires local/global resource source')
            result['resource'] = dict(scope=args[2].lower(), reference=symbol('resource', args[3]))
        elif category == 'uievent':
            result['arguments'] = args[2:]
        else:
            require(len(args) == 2, 'unexpected positional event arguments')
        for name, values, _ in options:
            modifier = dict(operation=name)
            if name == 'resource':
                require(values[0].lower() in {'local', 'global'}, 'event resource scope outside observed contract')
                modifier.update(scope=values[0].lower(), reference=symbol('resource', values[1]))
            result['modifiers'].append(modifier)
        return result
    return None


def numeric_text(value):
    return value['text'] if value['kind'] == 'externalVariable' else str(value['value'])


def render(node):
    """Render the typed domain fields for independent AEB comparison."""
    domain = node.get('domain')
    if not domain:
        return node['text']
    kind, command = domain['kind'], node['command']
    if kind == 'applicationCount':
        if domain['mode'] == 'resourceBounded':
            return 'maxApplyCount'
        return 'applyCount ' + numeric_text(domain['minimum']) + ' ' + numeric_text(domain['maximum'])
    if kind in {'rate', 'priority'}:
        return command + ' ' + numeric_text(domain['value'])
    if kind == 'controlFlags':
        return command + ' ' + ' '.join('-' + ('if' if o['operation'] == 'test' else o['operation']) +
                                       ' ' + ' '.join(s['name'] for s in o['symbols']) for o in domain['operations'])
    if kind == 'timeTrigger':
        result = 'timeTrigger ' + domain['period']['name'] + ' ' + numeric_text(domain['time'])
        for modifier in domain['modifiers']:
            result += ' -' + modifier['operation']
            if modifier['operation'] != 'fromcreate':
                result += ' ' + numeric_text(modifier['value'])
        return result
    if kind == 'transportOptions':
        result = 'options'
        for op in domain['operations']:
            result += ' -' + op['operation']
            if 'destination' in op:
                result += ' ' + op['destination']['name']
            if 'reference' in op:
                result += ' ' + op['reference']['name']
            if 'life' in op:
                result += ' ' + numeric_text(op['life'])
            if 'regionRaw' in op:
                result += ' ' + op['regionRaw']
        return result
    if kind == 'ruleEdge':
        result = command + (' ' + domain['target']['name'] if domain['target'] else '')
        for action in domain['actions']:
            result += ' -createUnit ' + action['unit']['name'] + ' (' + ', '.join(numeric_text(v) for v in action['offset']) + ')'
        return result
    if kind == 'agentTest':
        return 'testAgent ' + domain['handler']['name'] + ' -canReach ' + domain['canReach']['name']
    # Event payloads remain typed and retained; binary checks validate the event
    # header separately. Rendering arbitrary UI payload quoting is not implemented.
    return node['text']

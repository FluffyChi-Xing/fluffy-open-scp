"""Source-preserving research scanner for the verified ArgScript argument layer.

Native evidence: StripCommentsAndWhiteSpace / SplitIntoArguments in the local
development decompilation. No shell escaping, substitution or file inclusion.
Offsets/columns are Python Unicode code points; ends are exclusive.
"""
import hashlib
import re


def diagnostic(code, message, file, line, column, **extra):
    return dict(code=code, message=message, file=file, line=line, column=column, **extra)


def strip_line(raw, offset, block_stack, file, line):
    """Remove comments while retaining each emitted character's original offset."""
    chars, positions, comments, errors = [], [], [], []
    i = 0
    while i < len(raw):
        char = raw[i]
        if char in '\"\'\\':
            end = raw.find(char, i + 1)
            end = len(raw) if end < 0 else end + 1
            if not block_stack:
                chars.extend(raw[i:end]); positions.extend(range(offset + i, offset + end))
            i = end
        elif raw.startswith('#<', i):
            block_stack.append(dict(file=file, line=line, column=i + 1))
            comments.append(dict(kind='block-open', start=offset + i, end=offset + i + 2))
            i += 2
        elif raw.startswith('#>', i):
            if not block_stack:
                errors.append(diagnostic('AS001', "'#>' without matching '#<'", file, line, i + 1))
            else:
                block_stack.pop()
            comments.append(dict(kind='block-close', start=offset + i, end=offset + i + 2))
            i += 2
        elif char == '#':
            comments.append(dict(kind='line', start=offset + i, end=offset + len(raw)))
            break
        else:
            if not block_stack:
                chars.append(char); positions.append(offset + i)
            i += 1
    return ''.join(chars), positions, comments, errors


def segments(positions):
    result = []
    for position in positions:
        if result and result[-1]['end'] == position:
            result[-1]['end'] += 1
        else:
            result.append(dict(start=position, end=position + 1))
    return result


def split_arguments(text, positions, raw, offset, file, line):
    tokens, errors = [], []
    i = 0
    while i < len(text):
        if text[i].isspace():
            i += 1; continue
        start = i
        if text[i] == ')':
            errors.append(diagnostic('AS003', "unexpected ')'", file, line, positions[i] - offset + 1))
            i += 1; continue
        kind = 'word'
        if text[i] in '(\"':
            kind = 'group' if text[i] == '(' else 'quoted'
            opening = text[i]
            depth = 1 if opening == '(' else 0
            i += 1
            while i < len(text):
                if text[i] == '\"':
                    if opening == '\"':
                        break
                    end = text.find('\"', i + 1)
                    if end < 0:
                        errors.append(diagnostic('AS002', 'missing end quote', file, line, positions[i] - offset + 1))
                        i = len(text); break
                    i = end + 1; continue
                if opening == '(':
                    if text[i] == '(':
                        depth += 1
                    elif text[i] == ')':
                        depth -= 1
                        if depth == 0:
                            break
                i += 1
            if i >= len(text):
                code = 'AS002' if opening == '\"' else 'AS004'
                if not errors or errors[-1]['code'] != 'AS002':
                    errors.append(diagnostic(code, 'missing closing delimiter', file, line, positions[start] - offset + 1))
                break
            i += 1
            value = text[start + 1:i - 1]
        else:
            while i < len(text) and not text[i].isspace() and text[i] not in '(\"':
                if text[i] == ')':
                    break
                i += 1
            value = text[start:i]
        if not value and i == start:
            i += 1; continue
        source_segments = segments(positions[start:i])
        first, last = positions[start], positions[i - 1] + 1
        is_option = kind == 'word' and len(value) > 1 and value[0] == '-' and value[1].isascii() and value[1].isalpha()
        if kind == 'word' and value == '-':
            errors.append(diagnostic('AS005', 'option has no label', file, line, first - offset + 1))
        tokens.append(dict(kind='option' if is_option else kind, value=value,
                           text=text[start:i], raw=raw[first - offset:last - offset],
                           span=dict(file=file, line=line, column=first - offset + 1,
                                     endColumn=last - offset + 1, start=first, end=last),
                           sourceSegments=source_segments))
    return tokens, errors


def argument_view(tokens):
    """Options remain ordered and repeated; quoted '-or' is a plain argument."""
    main, options = [], []
    for token in tokens:
        if token['kind'] == 'option':
            options.append(dict(name=token['value'][1:], arguments=[], token=token))
        elif options:
            options[-1]['arguments'].append(token)
        else:
            main.append(token)
    return dict(main=main, options=options)


def scan(text, file='<input>'):
    block_stack, lines, diagnostics = [], [], []
    offset = 0
    physical_lines = [m.group() for m in re.finditer(r'[^\r\n]*(?:\r\n|\r|\n|$)', text) if m.group()]
    for number, raw_line in enumerate(physical_lines, 1):
        raw = raw_line.rstrip('\r\n')
        clean, positions, comments, errors = strip_line(raw, offset, block_stack, file, number)
        tokens, token_errors = split_arguments(clean, positions, raw, offset, file, number)
        lines.append(dict(number=number, start=offset, raw=raw_line, clean=clean,
                          positions=positions, comments=comments, tokens=tokens,
                          arguments=argument_view(tokens)))
        diagnostics.extend(errors + token_errors)
        offset += len(raw_line)
    for opening in block_stack:
        diagnostics.append(diagnostic('AS006', 'unclosed block comment', **opening))
    return dict(schema='openscp.argscript.source/0', file=file, source=text,
                sha256=hashlib.sha256(text.encode('utf-8')).hexdigest(), lines=lines,
                diagnostics=diagnostics,
                limitations=['No substitution, macro execution or include resolution',
                             'Raw scanning diagnostics may belong to inactive/deferred meta text',
                             'ASCII option labels verified; Unicode native ctype compatibility unverified'])

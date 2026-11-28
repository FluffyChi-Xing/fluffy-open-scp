"""Source-derived parser contracts; no retail binaries or live game required."""
import unittest

from argscript_parser import Parser
from argscript_syntax import scan
from argscript_variables import (ArgScriptError, Variables, pop_namespace,
                                push_namespace, set_global_variable, set_variable,
                                set_variable_raw, substitute, variable)


class VariableContracts(unittest.TestCase):
    def test_scope_lookup_walks_actual_push_offsets_and_root(self):
        root = set_variable_raw(Variables(), 'x', 'root')
        parent = set_variable_raw(push_namespace(root, 'A'), 'x', 'parent')
        child = push_namespace(parent, 'B:C')
        self.assertEqual(variable(child, 'X'), 'parent')
        self.assertEqual(variable(child, ':X'), 'root')
        child = set_variable_raw(child, 'X', 'child')
        self.assertEqual(variable(child, 'x'), 'child')
        self.assertEqual(variable(root, 'x'), 'root')  # pure updates
        self.assertEqual(pop_namespace(child).scope, 'A:')

    def test_pop_does_not_delete_bindings(self):
        state = set_variable_raw(push_namespace(Variables(), 'A'), 'x', 'a')
        state = pop_namespace(state)
        self.assertIsNone(variable(state, 'x'))
        self.assertEqual(variable(state, 'A:x'), 'a')

    def test_setter_does_not_interpret_leading_colon_as_root(self):
        state = set_variable_raw(push_namespace(Variables(), 'A'), ':x', 'odd')
        self.assertIsNone(variable(state, ':x'))
        self.assertEqual(variable(state, ':A::x'), 'odd')

    def test_global_fallback_is_exact_case_then_external(self):
        state = Variables(external_values=(('X', 'external'),))
        state = set_global_variable(state, 'X', 'global')
        self.assertEqual(variable(state, 'X'), 'global')
        self.assertIsNone(variable(state, 'x'))
        state = set_global_variable(state, 'X', None)
        self.assertEqual(variable(state, 'X'), 'external')
        state = set_variable_raw(state, 'x', 'local')
        self.assertEqual(variable(state, 'X'), 'local')

    def test_braced_interpolation_and_unbraced_group(self):
        state = set_variable_raw(Variables(), 'x', 'a b')
        self.assertEqual(substitute(state, '$x ${x}_tail')['text'], '(a b) a b_tail')
        self.assertEqual(substitute(state, '$x', False)['text'], 'a b')

    def test_substitution_also_occurs_inside_quotes_and_backslashes(self):
        state = set_variable_raw(Variables(), 'x', 'v')
        self.assertEqual(substitute(state, '"$x" \\$x')['text'], '"(v)" \\(v)')

    def test_inserted_dollars_not_recursively_expanded(self):
        state = set_variable_raw(Variables(), 'x', '$missing')
        self.assertEqual(substitute(state, '$x')['text'], '($missing)')
        assigned = set_variable(state, 'y', '$x')
        self.assertEqual(variable(assigned, 'y'), '$missing')

    def test_errors_distinguish_name_brace_lookup(self):
        for text, code in [('$', 'ASV002'), ('${', 'ASV002'), ('$1', 'ASV003'),
                           ('$$x', 'ASV003'), ('${x', 'ASV004'), ('$unknown', 'ASV005')]:
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                substitute(Variables(), text)
            self.assertEqual(caught.exception.code, code)

    def test_original_input_limit_only_when_dollar_present(self):
        state = set_variable_raw(Variables(), 'x', 'a' * 3000)
        self.assertEqual(len(substitute(state, 'q' * 2045 + '$x')['text']), 5047)
        with self.assertRaises(ArgScriptError) as caught:
            substitute(state, 'q' * 2046 + '$x')
        self.assertEqual(caught.exception.code, 'ASV001')
        self.assertEqual(len(substitute(state, 'q' * 4000)['text']), 4000)

    def test_pieces_identify_reference_and_literal(self):
        state = set_variable_raw(Variables(), 'x', 'abc')
        result = substitute(state, 'a ${x} z')
        self.assertEqual(result['pieces'][1], dict(start=2, end=5, inputStart=2,
                         inputEnd=6, kind='variable', name='x'))

    def test_empty_value_is_distinct_from_missing(self):
        state = set_variable_raw(Variables(), 'x', '')
        self.assertEqual(substitute(state, '$x/${x}')['text'], '()/')

    def test_explicit_host_guards(self):
        with self.assertRaises(ArgScriptError):
            pop_namespace(Variables())
        with self.assertRaises(ArgScriptError):
            push_namespace(Variables(), '非ASCII')
        with self.assertRaises(ArgScriptError):
            substitute(Variables(), '文字 $x')


class Collector:
    def __init__(self, consume=True):
        self.lines, self.ends, self.consume = [], [], consume

    def parse(self, parser, text, origin):
        self.lines.append(text)
        return self.consume

    def end(self, parser, error):
        self.ends.append(error)


class ParserContracts(unittest.TestCase):
    def parser(self):
        parser, calls = Parser(), []
        parser.register('emit', lambda p, invocation: calls.append(invocation))
        return parser, calls

    def test_substitute_before_split_with_different_interpolation_forms(self):
        parser, calls = self.parser()
        parser.variables = set_variable_raw(parser.variables, 'x', 'a b')
        parser.parse_stream('emit $x ${x} -repeat A -repeat B')
        args = calls[0]['arguments']
        self.assertEqual([t['value'] for t in args['main']], ['a b', 'a', 'b'])
        self.assertEqual([o['name'] for o in args['options']], ['repeat', 'repeat'])

    def test_injected_comment_markers_are_not_restripped(self):
        parser, calls = self.parser()
        parser.variables = set_variable_raw(parser.variables, 'x', '# literal')
        parser.parse_stream('emit ${x}')
        self.assertEqual([t['value'] for t in calls[0]['arguments']['main']], ['#', 'literal'])

    def test_command_name_can_be_substituted(self):
        parser, calls = self.parser()
        parser.variables = set_variable_raw(parser.variables, 'command', 'EMIT')
        parser.parse_stream('${command} yes')
        self.assertEqual(calls[0]['command'], 'EMIT')

    def test_inner_block_outer_block_global_lookup_and_replacement(self):
        parser, calls = self.parser()
        parser.start_block({'EMIT': lambda p, i: calls.append('outer')})
        parser.start_block({'other': lambda p, i: None})
        parser.parse_line('EmIt')
        parser.end_block(); parser.end_block()
        parser.register('EMIT', lambda p, i: calls.append('replacement'))
        parser.parse_line('emit')
        self.assertEqual(calls, ['outer', 'replacement'])

    def test_meta_consumption_precedes_substitution_and_argument_errors(self):
        parser, calls = self.parser()
        meta = Collector()
        parser.register('define', lambda p, i: p.start_meta(meta, 'enddef'))
        parser.parse_stream('define\nemit $missing (\n ENDDEF # comment\n')
        self.assertEqual(meta.lines, ['emit $missing ('])
        self.assertEqual(meta.ends, [False])
        self.assertEqual(calls, [])

    def test_meta_end_matches_whole_line_before_substitution(self):
        parser, _ = self.parser()
        meta = Collector()
        parser.start_meta(meta, 'enddef')
        parser.parse_line('enddef extra')
        parser.parse_line('${close}')
        self.assertEqual(meta.lines, ['enddef extra', '${close}'])
        parser.parse_line('ENDDEF')
        self.assertEqual(meta.ends, [False])

    def test_meta_pass_through_reaches_dispatch(self):
        parser, calls = self.parser()
        parser.start_meta(Collector(consume=False), 'done')
        parser.parse_line('emit a')
        self.assertEqual(len(calls), 1)

    def test_direct_arguments_bypass_substitution_and_meta(self):
        parser, calls = self.parser()
        parser.start_meta(Collector(), 'done')
        parser.parse_arguments(scan('emit $missing')['lines'][0]['tokens'])
        self.assertEqual(calls[0]['arguments']['main'][0]['value'], '$missing')

    def test_stream_balance_relative_to_entry_depth(self):
        parser, _ = self.parser()
        parser.start_block({})
        parser.parse_stream('emit fine')
        parser.register('open', lambda p, i: p.start_block({}))
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream('open')
        self.assertEqual(caught.exception.code, 'ASP002')
        parser.finish(True)
        self.assertEqual(parser.blocks, [])

    def test_stream_cannot_silently_close_enclosing_meta(self):
        parser, _ = self.parser()
        meta = Collector()
        parser.start_meta(meta, 'done')
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream('done')
        self.assertEqual(caught.exception.code, 'ASP003')
        self.assertEqual(meta.ends, [False])

    def test_unclosed_meta_and_error_cleanup_order(self):
        parser, _ = self.parser()
        events = []
        parser.start_block({}, lambda p, error: events.append(('block', error)))
        meta = Collector()
        parser.register('open', lambda p, i: p.start_meta(meta, 'done'))
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream('open\n$not_resolved (')
        self.assertEqual(caught.exception.code, 'ASP003')
        parser.finish(True)
        self.assertEqual(meta.ends, [True])
        self.assertEqual(events, [('block', True)])

    def test_source_mapping_does_not_invent_value_source_positions(self):
        parser, calls = self.parser()
        parser.variables = set_variable_raw(parser.variables, 'x', 'long expanded value')
        source = '  emit ${x}#< hidden #>tail\r\n'
        parser.parse_stream(source, 'mapping.er2')
        tokens = calls[0]['arguments']['main']
        last = tokens[-1]
        refs = last['sourceReferences']
        self.assertEqual([source[r['start']:r['end']] for r in refs], ['${x}', 'tail'])
        self.assertNotIn('span', last)
        self.assertIn('generatedSpan', last)

    def test_failure_maps_back_through_comments_and_crlf_and_stops(self):
        parser, calls = self.parser()
        source = 'emit ok\r\n  emit #< removed #> $missing\r\nemit never'
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream(source)
        error = caught.exception
        self.assertEqual(source[error.start:error.end], '$missing')
        self.assertEqual((error.file, error.line, error.phase), ('<input>', 2, 'substitution'))
        self.assertEqual(parser.trace[-1]['failure']['code'], 'ASV005')
        self.assertEqual(len(calls), 1)

    def test_unknown_command_maps_expansion_to_reference(self):
        parser, _ = self.parser()
        parser.variables = set_variable_raw(parser.variables, 'x', 'unknown')
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream('  ${x}')
        self.assertEqual((caught.exception.start, caught.exception.end), (2, 6))

    def test_finish_preserves_variables_and_resets_scope_and_comments(self):
        parser, _ = self.parser()
        parser.variables = set_variable_raw(push_namespace(parser.variables, 'A'), 'x', 'v')
        parser.parse_line('#<')
        self.assertEqual(parser.finish(), 1)
        self.assertEqual(parser.variables.scope, '')
        self.assertEqual(variable(parser.variables, 'A:x'), 'v')
        self.assertEqual(parser.comment_stack, [])


if __name__ == '__main__':
    unittest.main()

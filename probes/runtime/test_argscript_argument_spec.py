"""cArgumentSpec source contracts and integration through text entry point."""
import unittest

from argscript_argument_spec import (ArgInfo, ArgumentSpec, ConversionRequest,
                                    EnumSpec, OptionSpec, REPEAT, VECTOR,
                                    checked_command, dependency_slots, parse_argv, parse_invocation)
from argscript_parser import Parser
from argscript_variables import ArgScriptError, set_variable_raw


def string(name, **kwargs):
    return ArgInfo(name, destination=name, **kwargs)


def values(result):
    return [write['value'] for write in result.writes]


class DependencyContracts(unittest.TestCase):
    def test_optional_group_starts_optional_then_becomes_dependent(self):
        infos = dependency_slots([('required', string('r')), ('[a', string('a')),
                                  ('b]', string('b'))])
        spec = ArgumentSpec(arguments=infos)
        self.assertEqual([i.dependent for i in infos], [True, False, True])
        self.assertEqual(parse_argv(spec, ['cmd', 'r']).native_code, 0)
        missing = parse_argv(spec, ['cmd', 'r', 'a'])
        self.assertEqual((missing.native_code, missing.message), (2, 'Not enough arguments'))
        self.assertEqual(values(missing), ['r', 'a'])
        self.assertEqual(parse_argv(spec, ['cmd', 'r', 'a', 'b']).native_code, 0)

    def test_independent_optional_and_nested_groups(self):
        infos = dependency_slots([('[a]', string('a')), ('[b', string('b')),
                                  ('[c]]', string('c'))])
        self.assertEqual([i.dependent for i in infos], [False, False, False])
        self.assertEqual(parse_argv(ArgumentSpec(arguments=infos), ['cmd']).native_code, 0)

    def test_ellipsis_sets_both_bits_and_must_be_last(self):
        infos = dependency_slots([('x', string('x')), ('...', None)])
        self.assertEqual(infos[0].type_id, 6 | VECTOR | REPEAT)
        for rows in [[('...', None)], [('x', string('x')), ('...', None), ('y', string('y'))]]:
            with self.assertRaises(ArgScriptError) as caught:
                dependency_slots(rows)
            self.assertEqual(caught.exception.code, 'ASC002')

    def test_unbalanced_brackets_rejected(self):
        with self.assertRaises(ArgScriptError) as caught:
            dependency_slots([('[x', string('x'))])
        self.assertEqual(caught.exception.code, 'ASC004')


class ConsumptionContracts(unittest.TestCase):
    def test_argv_includes_command_but_it_is_not_a_default_argument(self):
        result = parse_argv(ArgumentSpec(arguments=(string('x'),)), ['cmd', 'value'])
        self.assertEqual(values(result), ['value'])
        self.assertEqual(result.writes[0]['argvIndex'], 1)
        self.assertTrue(result.fully_converted)

    def test_option_fixed_arity_leaves_remaining_values_for_main(self):
        spec = ArgumentSpec(arguments=(string('main'),),
                            options=(OptionSpec('o', (string('opt'),)),))
        result = parse_argv(spec, ['cmd', '-o', 'option-value', 'main-value'])
        self.assertEqual(result.native_code, 0)
        self.assertEqual([w['destination'] for w in result.writes], ['opt', 'main'])

    def test_options_and_main_can_interleave(self):
        spec = ArgumentSpec(arguments=(string('a'), string('b')),
                            options=(OptionSpec('o', (string('opt'),)),))
        result = parse_argv(spec, ['cmd', 'first', '-O', 'option', 'second'])
        self.assertEqual(values(result), ['first', 'option', 'second'])

    def test_negative_numbers_are_values_and_dash_alpha_is_option(self):
        spec = ArgumentSpec(arguments=(string('a'), string('b')))
        self.assertEqual(values(parse_argv(spec, ['cmd', '-3', '-.5'])), ['-3', '-.5'])
        error = parse_argv(spec, ['cmd', '-missing'])
        self.assertEqual((error.native_code, error.message), (5, "Unknown option 'missing'"))

    def test_repeated_scalar_options_write_in_order_and_flags_are_or(self):
        spec = ArgumentSpec(options=(OptionSpec('x', (string('x', flag=4),), flag=2),))
        result = parse_argv(spec, ['cmd', '-x', 'a', '-X', 'b'])
        self.assertEqual(values(result), ['a', 'b'])
        self.assertEqual(result.flags, (1 << 2) | (1 << 4))
        self.assertFalse(any(w['append'] for w in result.writes))
        self.assertEqual(parse_argv(spec, ['cmd']).flags, 0)

    def test_option_flag_is_set_before_missing_argument_failure(self):
        spec = ArgumentSpec(options=(OptionSpec('o', (string('x'), string('y')), flag=3),))
        result = parse_argv(spec, ['cmd', '-o'])
        self.assertEqual(result.native_code, 2)
        self.assertEqual(result.message, 'Not enough arguments: expecting at least 2 more in -o')
        self.assertEqual(result.flags, 8)

    def test_optional_option_group_may_be_omitted(self):
        infos = dependency_slots([('[a', string('a')), ('b]', string('b'))])
        spec = ArgumentSpec(options=(OptionSpec('o', infos), OptionSpec('next')))
        self.assertEqual(parse_argv(spec, ['cmd', '-o', '-next']).native_code, 0)
        bad = parse_argv(spec, ['cmd', '-o', 'a', '-next'])
        self.assertEqual(bad.message, 'Not enough arguments: expecting at least 1 more in -o')

    def test_variadic_main_can_be_empty_but_required_option_variadic_cannot(self):
        repeated = string('x', type_id=6 | VECTOR | REPEAT)
        self.assertEqual(parse_argv(ArgumentSpec(arguments=(repeated,)), ['cmd']).native_code, 0)
        spec = ArgumentSpec(options=(OptionSpec('o', (repeated,)),))
        self.assertEqual(parse_argv(spec, ['cmd', '-o']).native_code, 2)
        result = parse_argv(spec, ['cmd', '-o', 'a', 'b'])
        self.assertEqual(values(result), ['a', 'b'])
        self.assertTrue(all(w['append'] for w in result.writes))

    def test_repeat_direct_vector_call_does_not_set_argument_flag(self):
        repeated = string('x', type_id=6 | VECTOR | REPEAT, flag=7)
        result = parse_argv(ArgumentSpec(arguments=(repeated,)), ['cmd', 'a'])
        self.assertEqual(result.flags, 0)
        self.assertEqual(values(result), ['a'])

    def test_vector_group_splits_and_appends_even_option_like_values(self):
        spec = ArgumentSpec(arguments=(string('x', type_id=6 | VECTOR, flag=7),))
        result = parse_argv(spec, ['cmd', 'a -literal "b c"'])
        self.assertEqual(values(result), ['a', '-literal', 'b c'])
        self.assertEqual(result.flags, 128)

    def test_variadic_does_not_resplit_each_item(self):
        repeated = string('x', type_id=6 | VECTOR | REPEAT)
        result = parse_argv(ArgumentSpec(arguments=(repeated,)), ['cmd', 'a b', 'c'])
        self.assertEqual(values(result), ['a b', 'c'])

    def test_help_can_override_declared_option_and_skip_missing_required(self):
        spec = ArgumentSpec(arguments=(string('required'),), options=(OptionSpec('help', flag=3),))
        result = parse_argv(spec, ['cmd', '-HeLp'])
        self.assertEqual((result.native_code, result.flags), (1, 0))
        disabled = ArgumentSpec(options=spec.options, respect_help=False)
        result = parse_argv(disabled, ['cmd', '-help'])
        self.assertEqual((result.native_code, result.flags), (0, 8))

    def test_too_many_and_not_enough_are_distinct(self):
        spec = ArgumentSpec(arguments=(string('one'),))
        self.assertEqual(parse_argv(spec, ['cmd']).native_code, 2)
        result = parse_argv(spec, ['cmd', 'a', 'b'])
        self.assertEqual(result.native_code, 3)
        self.assertEqual(values(result), ['a'])


class ConversionContracts(unittest.TestCase):
    def test_enum_case_insensitive_help_table_has_priority(self):
        spec = ArgumentSpec(arguments=(ArgInfo('mode', 12, destination='mode'),),
                            enums=(EnumSpec('Mode', (('on', 1),), (('ON', 9),)),))
        self.assertEqual(values(parse_argv(spec, ['cmd', 'On'])), [9])
        bad = parse_argv(spec, ['cmd', 'unknown'])
        self.assertEqual((bad.native_code, bad.message),
                         (6, "Unknown enum 'unknown' of type Mode"))

    def test_enum_failure_retains_prior_writes_and_current_argument_flag(self):
        spec = ArgumentSpec(arguments=(string('a'), ArgInfo('b', 12, destination='b', flag=2)),
                            enums=(EnumSpec('Mode', (('ok', 1),)),))
        result = parse_argv(spec, ['cmd', 'first', 'bad'])
        self.assertEqual((result.native_code, result.flags), (6, 4))
        self.assertEqual(values(result), ['first'])

    def test_parser_numeric_conversion_is_deferred_not_python_eval(self):
        spec = ArgumentSpec(arguments=(ArgInfo('n', 2, destination='n'),))
        result = parse_argv(spec, ['cmd', '2+3'])
        self.assertEqual(result.native_code, 0)
        self.assertEqual(values(result), [ConversionRequest('ParseInt', '2+3', 2)])
        self.assertFalse(result.fully_converted)

    def test_raw_and_parser_conversion_routes_differ(self):
        spec = ArgumentSpec(arguments=(ArgInfo('n', 2, destination='n'),))
        result = parse_argv(spec, ['cmd', '123suffix'], parser_present=False)
        self.assertEqual(result.unresolved[0].route, 'AtoI64')
        repeated = ArgInfo('n', 2 | VECTOR | REPEAT, destination='n')
        result = parse_argv(ArgumentSpec(arguments=(repeated,)), ['cmd', 'x'], parser_present=False)
        self.assertEqual(result.unresolved[0].route, 'StrtoI32')

    def test_double_uses_raw_conversion_even_with_parser(self):
        spec = ArgumentSpec(arguments=(ArgInfo('n', 4, destination='n'),))
        result = parse_argv(spec, ['cmd', '2+3'])
        self.assertEqual(result.unresolved[0].route, 'StrtodEnglish.float64')

    def test_explicit_converter_only_called_for_destination(self):
        seen = []
        def converter(text):
            seen.append(text)
            return 42
        spec = ArgumentSpec(arguments=(ArgInfo('skip', 2), ArgInfo('n', 2, destination='n')))
        result = parse_argv(spec, ['cmd', 'ignored', 'expr'], converters={'ParseInt': converter})
        self.assertEqual(seen, ['expr'])
        self.assertEqual(values(result), [42])
        self.assertTrue(result.fully_converted)

    def test_unknown_type_and_parser_only_vector_type(self):
        spec = ArgumentSpec(arguments=(ArgInfo('v', 7, destination='v'),))
        self.assertEqual(parse_argv(spec, ['cmd', '1,2'], parser_present=False).native_code, 4)
        parsed = parse_argv(spec, ['cmd', '1,2'])
        self.assertEqual(parsed.unresolved[0].route, 'ParseVector2')


class PipelineContracts(unittest.TestCase):
    def test_text_entry_retains_raw_argv_after_chomp_and_reclassifies_quotes(self):
        parser, seen = Parser(), []
        spec = ArgumentSpec()
        parser.register('cmd', lambda p, i: seen.append(parse_invocation(spec, i)))
        parser.parse_stream('cmd "-HeLp"')
        invocation = parser.trace[-1]['invocation']
        self.assertEqual([t['value'] for t in invocation['argv']], ['cmd', '-HeLp'])
        self.assertEqual([t['value'] for t in invocation['arguments']['main']], ['-HeLp'])
        self.assertEqual(seen[0].native_code, 1)
        self.assertTrue(invocation['allOptionsHandled'])

    def test_text_variable_option_fixed_arity_and_main_continuation(self):
        parser, seen = Parser(), []
        parser.variables = set_variable_raw(parser.variables, 'n', 'one two')
        spec = ArgumentSpec(arguments=(string('a'),), options=(OptionSpec('x', (string('x'),)),))
        parser.register('cmd', lambda p, i: seen.append(parse_invocation(spec, i)))
        parser.parse_stream('cmd -x $n tail')
        self.assertEqual(values(seen[0]), ['one two', 'tail'])
        self.assertTrue(seen[0].fully_converted)

    def test_wrapper_marks_options_even_on_unknown_option(self):
        parser = Parser()
        parser.register('cmd', lambda p, i: parse_invocation(ArgumentSpec(), i))
        parser.parse_stream('cmd -unknown')
        invocation = parser.trace[-1]['invocation']
        self.assertTrue(invocation['allOptionsHandled'])
        self.assertEqual(invocation['argumentSpecResult'].native_code, 5)

    def test_checked_adapter_stops_consumer_and_keeps_raw_failure(self):
        parser, consumed = Parser(), []
        parser.register('cmd', checked_command(ArgumentSpec(),
                        lambda p, i, result: consumed.append(result)))
        source = 'cmd #< removed #> -unknown\r\ncmd'
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream(source, 'failure.er2')
        error = caught.exception
        self.assertEqual((error.native_code, error.line, error.phase), (5, 1, 'dispatch'))
        self.assertEqual(source[error.start:error.end], '-unknown')
        self.assertEqual(consumed, [])
        self.assertEqual(len(parser.trace), 1)
        self.assertEqual(parser.trace[-1]['invocation']['argumentSpecResult'].native_code, 5)

    def test_checked_adapter_blocks_unresolved_conversion(self):
        parser, consumed = Parser(), []
        spec = ArgumentSpec(arguments=(ArgInfo('n', 2, destination='n'),))
        parser.register('cmd', checked_command(spec, lambda p, i, r: consumed.append(r)))
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream('cmd (2+3)')
        self.assertEqual(caught.exception.code, 'ASC009')
        self.assertEqual(caught.exception.native_code, 0)
        self.assertEqual((caught.exception.start, caught.exception.end), (4, 9))
        self.assertEqual(consumed, [])

    def test_checked_adapter_success_runs_consumer(self):
        parser, consumed = Parser(), []
        parser.register('cmd', checked_command(ArgumentSpec(arguments=(string('s'),)),
                        lambda p, i, r: consumed.extend(values(r))))
        parser.parse_stream('cmd good')
        self.assertEqual(consumed, ['good'])


if __name__ == '__main__':
    unittest.main()

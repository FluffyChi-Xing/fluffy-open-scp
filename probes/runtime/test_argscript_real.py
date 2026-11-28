"""Real conversion source contracts and float-width regression examples."""
import math
import unittest

from argscript_argument_spec import ArgInfo, ArgumentSpec, VECTOR, checked_command, parse_argv
from argscript_parser import Parser
from argscript_real import float32, numeric_converters, parse_real, parse_vector, read_real
from argscript_variables import ArgScriptError, set_variable_raw


class RawRealContracts(unittest.TestCase):
    def test_decimal_prefix_end_and_incomplete_exponent(self):
        for text, expected, end in [('  -12.5tail', -12.5, 7), ('1e+', 1., 3),
                                    ('1E-2suffix', .01, 4), ('e2', 0., 2),
                                    ('.', 0., 1), (' +bad', 0., 2), ('nan', 0., 0)]:
            with self.subTest(text=text):
                parsed = read_real(text)
                self.assertEqual((parsed.value, parsed.end), (expected, end))

    def test_fraction_accumulates_digitwise_instead_of_using_python_float(self):
        self.assertEqual(read_real('0.3').value, 3 * .1)
        self.assertNotEqual(read_real('0.3').value, float('0.3'))

    def test_positive_negative_zero_and_overflow(self):
        self.assertEqual(math.copysign(1, read_real('-0').value), -1)
        self.assertEqual(read_real('1e400').value, math.inf)
        self.assertEqual(read_real('1e-400').value, 0.)
        self.assertTrue(math.isnan(read_real('0e400').value))

    def test_exponent_accumulator_is_signed_int32(self):
        self.assertEqual(read_real('1e4294967296').value, 1.)
        self.assertEqual(read_real('1e2147483648').value, 0.)

    def test_raw_double_and_float_routes_preserve_different_widths(self):
        converters = numeric_converters()
        double = converters['StrtodEnglish.float64']('16777217tail')
        single = converters['StrtodEnglish.float32']('16777217tail')
        self.assertEqual((double, single), (16777217., 16777216.))

    def test_unsupported_encoding_and_embedded_nul(self):
        for text in ('１.２', '1\0more'):
            with self.assertRaises(ArgScriptError):
                read_real(text)


class RealExpressionContracts(unittest.TestCase):
    def test_arithmetic_power_and_unary_precedence(self):
        for text, expected in [('2+3*4', 14.), ('(2+3)*4', 20.), ('2^3^2', 512.),
                                ('-2^2', 4.), ('2^-3', .125), ('1e2+2.5', 102.5)]:
            with self.subTest(text=text):
                self.assertEqual(parse_real(text), expected)

    def test_float32_operand_and_term_stores_are_observable(self):
        self.assertEqual(parse_real('16777217-16777216'), 0.)
        self.assertEqual(parse_real('16777217*0'), 0.)
        self.assertEqual(parse_real('16777216+1-16777216'), 0.)
        self.assertEqual(parse_real('16777217/16777216'), 1.)

    def test_sse_remainder_differs_from_fmod_for_negative_lhs(self):
        self.assertEqual(parse_real('-7%3'), 2.)
        self.assertEqual(parse_real('7%-3'), -2.)
        self.assertEqual(parse_real('fmod(-7,3)'), -1.)
        self.assertEqual(parse_real('remainder(-7,3)'), 2.)

    def test_functions_and_prefix_order(self):
        # Compare checks only the input length: sqr matches the earlier sqrt.
        self.assertEqual(parse_real('abs(-3)+sqr(4)'), 5.)
        self.assertEqual(parse_real('sqrt(9)'), 3.)
        self.assertEqual(parse_real('s(9)'), 3.)  # sqrt precedes sqr.
        self.assertEqual(parse_real('pow(2,3)'), 8.)
        self.assertEqual(parse_real('floor(-1.2)+ceil(1.2)'), 0.)
        self.assertEqual(parse_real('round(-1.5)'), -1.)
        self.assertAlmostEqual(parse_real('frac(-1.25)'), .75)

    def test_ieee_special_results_are_not_python_arithmetic_exceptions(self):
        self.assertEqual(parse_real('1/0'), math.inf)
        self.assertEqual(parse_real('-1/0'), -math.inf)
        self.assertTrue(math.isnan(parse_real('0/0')))
        self.assertTrue(math.isnan(parse_real('sqrt(-1)')))
        self.assertTrue(math.isnan(parse_real('(-1)^0.5')))
        self.assertTrue(math.isnan(parse_real('1%0')))

    def test_complete_consumption_and_empty_errors(self):
        for text, message in [('', 'empty expression'), (' ', 'Bad real expression'),
                               ('1.5tail', 'Garbage at end of real expression'),
                               ('(1.5', "Expected ')'"), ('1+', 'Bad real expression')]:
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                parse_real(text)
            self.assertEqual(str(caught.exception), message)
        self.assertEqual(parse_real(' 1e+ '), 1.)

    def test_unrecovered_constants_and_registered_callbacks_are_explicit(self):
        for text in ('pi', 'sin(0)', 'custom(1)', 'e'):
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                parse_real(text)
            self.assertEqual(caught.exception.code, 'ASR003')


class VectorContracts(unittest.TestCase):
    def test_scalar_broadcast_and_comma_components(self):
        self.assertEqual(parse_vector('2+3', 2), (5., 5.))
        self.assertEqual(parse_vector('2+3', 3), (5., 5., 5.))
        self.assertEqual(parse_vector('2+3', 4), (5., 5., 5., 5.))
        self.assertEqual(parse_vector('1,2,3', 3), (1., 2., 3.))

    def test_incomplete_explicit_vector_does_not_broadcast(self):
        for text, size in [('1,2', 3), ('1,2,3', 4), ('1,', 2)]:
            with self.subTest(text=text), self.assertRaises(ArgScriptError):
                parse_vector(text, size)

    def test_rgba_requires_rgb_and_defaults_alpha(self):
        self.assertEqual(parse_vector('.5,.25,1', 4, rgba=True), (.5, .25, 1., 1.))
        self.assertEqual(parse_vector('1,2,3,.5', 4, rgba=True), (1., 2., 3., .5))
        with self.assertRaises(ArgScriptError):
            parse_vector('1', 4, rgba=True)

    def test_native_last_component_has_no_eof_check(self):
        self.assertEqual(parse_vector('1,2,3,ignored', 3), (1., 2., 3.))
        self.assertEqual(parse_vector('1,2junk', 2), (1., 2.))
        with self.assertRaises(ArgScriptError):
            parse_vector('1junk', 2)
        with self.assertRaises(ArgScriptError):
            parse_vector('1,2,3junk', 4, rgba=True)

    def test_each_component_is_float32(self):
        self.assertEqual(parse_vector('16777217,1', 2), (16777216., 1.))


class RealPipelineContracts(unittest.TestCase):
    def test_scalar_double_fallback_does_not_evaluate_expression(self):
        spec = ArgumentSpec(arguments=(ArgInfo('f', 3, destination='f'),
                                       ArgInfo('d', 4, destination='d')))
        parsed = parse_argv(spec, ['cmd', '2+3', '2+3'], converters=numeric_converters())
        self.assertTrue(parsed.fully_converted)
        self.assertEqual([w['value'] for w in parsed.writes], [5., 2.])

    def test_vector_array_conversion_and_rgb_gap(self):
        spec = ArgumentSpec(arguments=(ArgInfo('v', 7 | VECTOR, destination='v'),))
        result = parse_argv(spec, ['cmd', '(1,2) (3,4)'], converters=numeric_converters())
        self.assertEqual([w['value'] for w in result.writes], [(1., 2.), (3., 4.)])
        rgb = parse_argv(ArgumentSpec(arguments=(ArgInfo('rgb', 10, destination='rgb'),)),
                         ['cmd', '1,2,3'], converters=numeric_converters())
        self.assertFalse(rgb.fully_converted)
        self.assertEqual(rgb.unresolved[0].route, 'ParseRGB')

    def test_text_variables_reach_vector_consumer(self):
        parser, writes = Parser(), []
        parser.variables = set_variable_raw(parser.variables, 'xyz', '1+2,4,5')
        spec = ArgumentSpec(arguments=(ArgInfo('v', 8, destination='v'),))
        parser.register('vec', checked_command(spec, lambda p, i, r: writes.extend(r.writes),
                                              converters=numeric_converters()))
        parser.parse_stream('vec $xyz')
        self.assertEqual(writes[0]['value'], (3., 4., 5.))

    def test_failed_vector_keeps_prior_values_and_stops_consumer(self):
        parser, consumed = Parser(), []
        spec = ArgumentSpec(arguments=(ArgInfo('f', 3, destination='f'),
                                       ArgInfo('v', 8, destination='v')))
        parser.register('cmd', checked_command(spec, lambda p, i, r: consumed.append(r),
                                              converters=numeric_converters()))
        source = 'cmd 1.5 (1,2)\r\ncmd 2 (1,2,3)'
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream(source)
        failure = parser.trace[-1]['invocation']['argumentSpecFailure']
        self.assertEqual(failure['partial']['writes'][0]['value'], 1.5)
        self.assertEqual(failure['route'], 'ParseVector3')
        self.assertEqual(source[caught.exception.start:caught.exception.end], '(1,2)')
        self.assertEqual(consumed, [])


if __name__ == '__main__':
    unittest.main()

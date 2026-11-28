"""Source-only corpus audit gates, using authored isolated AEC fixtures."""
from pathlib import Path
import tempfile
import unittest

from argscript_corpus_audit import audit, instance_id
from unit_rule_frontend import parse_expanded


class CorpusContracts(unittest.TestCase):
    def fixture(self, root, child=None):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        directory = Path(temporary.name)
        self.write(directory, '00000001', root)
        if child is not None:
            self.write(directory, instance_id('Child'), child)
        return directory

    def write(self, directory, identity, text):
        (directory / f'HeroesAndVillains-Scripts_257365692_{identity}.aec').write_text(text, encoding='utf-8')

    def test_include_macro_conditional_to_domain_ast(self):
        directory = self.fixture('include "Child.er2"\ncreate Make(CriminalRobber)',
             'define Make(kind)\nif (match(&{kind},"Criminal*"))\n'
             'unitRule Generated\napplyCount 1\nlocal Mail out 2\nend\nendif\nenddef')
        result = audit(directory, ['00000001'])
        self.assertTrue(result['strict']['complete'])
        self.assertEqual(len(result['strict']['macroCalls']), 1)
        self.assertEqual(len(result['includeAliases']), 1)
        ast = parse_expanded(result['strict']['rows'])
        self.assertEqual(ast['diagnostics'], [])
        self.assertEqual(ast['rules'][0]['name'], 'Generated')
        self.assertTrue(ast['rules'][0]['origin']['generated'])
        self.assertTrue(ast['rules'][0]['origin']['calls'])
        self.assertFalse(result['packageCompilationAllowed'])

    def test_external_symbol_is_preserved_never_assigned_a_guessed_number(self):
        directory = self.fixture('unitRule Timer\nrate ${GameHour}\nend')
        result = audit(directory, ['00000001'])
        self.assertEqual(result['strict']['failure']['code'], 'ASV005')
        self.assertTrue(result['symbolic']['complete'])
        self.assertEqual(result['symbolic']['externalBindings'], {'GameHour': '${GameHour}'})
        self.assertEqual(result['symbolic']['rows'][1]['text'], 'rate ${GameHour}')

    def test_symbolic_mode_does_not_guess_a_conditional_result(self):
        directory = self.fixture('if $MissingFlag\nunitRule R\nend\nendif')
        result = audit(directory, ['00000001'])
        self.assertFalse(result['symbolic']['complete'])
        self.assertEqual(result['symbolic']['failure']['code'], 'ASX004')

    def test_unknown_domain_command_is_not_auto_accepted(self):
        directory = self.fixture('unitRule R\nunknownCommand x\nend')
        result = audit(directory, ['00000001'])
        self.assertFalse(result['strict']['complete'])
        self.assertEqual(result['strict']['failure']['code'], 'ASP001')

    def test_unresolved_include_binding_is_a_hard_input_failure(self):
        directory = self.fixture('include "Absent.er2"')
        with self.assertRaises(ValueError) as caught:
            audit(directory, ['00000001'])
        self.assertIn('Unresolved source alias', str(caught.exception))


if __name__ == '__main__':
    unittest.main()

/* The entry a method id opens: the rule as written, its citations, its known bias and its
 * failure rate. */

import { $, state } from './state.js';
import { element } from './format.js';
import { findMethod } from './registry.js';
import { notice, boundValueText } from './analysis.js';

export function openDrawer(method, fallbackId, bound) {
  const drawer = $('method-drawer');
  const body = $('drawer-body');
  body.replaceChildren();
  $('drawer-title').textContent =
    method?.title || state.build.bindings.find((entry) => entry.id === fallbackId)?.title || fallbackId;

  if (!method) {
    const binding = state.build.bindings.find((entry) => entry.id === fallbackId);
    if (binding?.composed_from) {
      const base = findMethod(state.registry, binding.composed_from);
      body.append(
        notice(
          'warning',
          `A composition of ${binding.composed_from}`,
          'A method plus bound parameters, so it carries that entry\'s citations rather than its own. The binding travels in the fingerprint.',
        ),
      );
      if (base) {
        const open = element('button', 'button button--ghost button--small', 'Open the entry it composes');
        open.type = 'button';
        open.addEventListener('click', () => openDrawer(base));
        body.append(open);
      }
    } else {
      body.append(
        notice(
          'warning',
          'Not a registry entry',
          'No citation, no recorded bias and no failure rate under this id.',
        ),
      );
    }
    drawer.hidden = false;
    return;
  }

  const section = (heading, node) => {
    const wrap = element('section');
    wrap.append(element('h3', null, heading));
    wrap.append(node);
    return wrap;
  };

  const identity = element('dl', 'kv');
  const rows = [
    ['Registry id', method.id],
    ['Construct', method.construct],
    ['Status', method.status],
    ['Confidence', method.confidence],
    ['Debate', method.debate || 'not stated'],
  ];
  if (bound?.bound_parameters?.length) {
    rows.push(['Bound here', boundValueText(bound, ' = ').join(', ')]);
  }
  if (bound?.unread_parameters?.length) {
    rows.push(['Not taken by this rule', bound.unread_parameters.join(', ')]);
  }
  for (const [term, definition] of rows) identity.append(element('dt', null, term), element('dd', null, definition));
  body.append(section('Entry', identity));

  body.append(section('Rule', element('p', 'rule-text', method.rule.trim())));

  if (method.failure) {
    body.append(
      section(
        'Failure rate',
        notice(
          'danger',
          `${(method.failure.rate * 100).toFixed(1)} percent, ${method.failure.numerator} of ${method.failure.denominator}`,
          `${method.failure.definition}. Corpus ${method.failure.corpus}. Detectability ${method.failure.detectability}, so a bias figure for this rule averages working with not working.`,
        ),
      ),
    );
  }

  if (method.parameter?.length) {
    const list = element('ul');
    for (const parameter of method.parameter) {
      const item = element('li');
      item.append(element('strong', null, parameter.name));
      const published = parameter.published_values?.length ? ` Published values: ${parameter.published_values.join(', ')}.` : '';
      const chosen = parameter.default != null ? ` Default ${parameter.default} from ${parameter.default_source || 'an unnamed source'}.` : '';
      item.append(document.createTextNode(`${parameter.unit ? ` (${parameter.unit})` : ''}.${published}${chosen}`));
      if (parameter.notes) item.append(element('p', 'metric__note', parameter.notes.trim()));
      list.append(item);
    }
    body.append(section('Parameters', list));
  }

  if (method.citation?.length) {
    const list = element('ul');
    for (const citation of method.citation) {
      const item = element('li');
      item.append(element('strong', null, `${citation.role}: `));
      item.append(document.createTextNode(citation.reference));
      if (citation.doi) {
        const link = element('a', null, ` doi:${citation.doi}`);
        link.href = `https://doi.org/${citation.doi}`;
        link.rel = 'noreferrer';
        link.target = '_blank';
        item.append(link);
      }
      if (!citation.obtained) item.append(element('span', 'tag tag--decide', 'source not obtained'));
      list.append(item);
    }
    body.append(section('Citations', list));
  }

  if (method.bias?.length) {
    const list = element('ul');
    for (const bias of method.bias) {
      list.append(
        element(
          'li',
          null,
          `${bias.magnitude} ${bias.unit}${bias.direction ? ` (${bias.direction})` : ''} against ${bias.criterion} (${bias.criterion_kind})` +
            `${bias.source ? `, reported by ${bias.source}` : ''}${bias.conditional_on_success ? '. Conditional on the rule having worked.' : ''}`,
        ),
      );
    }
    body.append(section('Known bias', list));
  }

  if (method.disagrees_with?.length) {
    const list = element('ul');
    for (const other of method.disagrees_with) {
      const item = element('li', null, `${other.id} (${other.kind})`);
      // A reader choosing between two rules is choosing between two sources, and the id
      // alone does not say whose the other one is.
      const alternative = findMethod(state.registry, other.id);
      const source = alternative?.citation?.[0];
      if (source) {
        item.append(document.createTextNode(`, ${source.role}: ${source.reference}`));
      }
      if (other.note) item.append(document.createTextNode(`. ${other.note}`));
      if (alternative) {
        const open = element('button', 'button button--ghost button--small', 'Open this entry');
        open.type = 'button';
        open.addEventListener('click', () => openDrawer(alternative));
        item.append(document.createTextNode(' '));
        item.append(open);
      }
      list.append(item);
    }
    body.append(section('Disagrees with', list));
  }

  drawer.hidden = false;
}

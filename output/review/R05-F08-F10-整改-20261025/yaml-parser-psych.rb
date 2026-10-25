require 'yaml'
require 'json'
require 'digest'

rows = JSON.parse(File.read(ARGV.fetch(0))).select { |c| ['tabs', 'crlf-comment'].include?(c['name']) }.map do |c|
  values = ['original', 'output'].map do |key|
    text = c.fetch(key)
    begin
      stream = Psych.parse_stream(text)
      raise 'not one document' unless stream.children.length == 1
      root = stream.children.first.root
      raise 'root is not a mapping' unless root.is_a?(Psych::Nodes::Mapping)
      keys = root.children.each_slice(2).map { |pair| pair.first.value }
      raise 'prepend-rules is not unique' unless keys.count('prepend-rules') == 1
      [key, {status: 'PASS', sha256: Digest::SHA256.hexdigest(text), text: text,
             documents: stream.children.length, prepend_rules_keys: keys.count('prepend-rules')}]
    rescue StandardError => error
      [key, {status: 'FAIL', sha256: Digest::SHA256.hexdigest(text), text: text,
             error_class: error.class.name, reason: error.message}]
    end
  end.to_h
  checks = {}
  if values.values.all? { |result| result[:status] == 'PASS' }
    lines = c.fetch('output').lines
    first = lines.index { |line| line.strip == '# >>> claude-switcher pinned rules >>>' }
    last = lines.index { |line| line.strip == '# <<< claude-switcher pinned rules <<<' }
    managed = YAML.load("prepend-rules:\n" + lines[first..last].join).fetch('prepend-rules')
    domains = %w[claude.ai claude.com anthropic.com claudeusercontent.com challenges.cloudflare.com datadoghq.com segment.io stripe.com stripe.network stripecdn.com intercom.io intercomcdn.com intercomassets.com]
    rules = managed.map { |rule| rule.split(',', 3) }
    clean = (lines[0...first] + lines[(last + 1)..-1]).join
    checks = {
      thirteen_managed_rules: managed.length == 13 && rules.all? { |rule| rule.length == 3 && rule[0] == 'DOMAIN-SUFFIX' && rule[2] == c.fetch('group') } && rules.map { |rule| rule[1] }.sort == domains.sort,
      managed_precedes_user: YAML.load(c.fetch('output')).fetch('prepend-rules')[0, 13] == managed,
      exact_user_bytes: clean == c.fetch('original'),
      user_semantics: YAML.load(clean) == YAML.load(c.fetch('original')),
      changed_receipt: c.fetch('changed') == (c.fetch('output') != c.fetch('original')),
      actual_second_pass: c.fetch('second_output') == c.fetch('output') && c.fetch('second_changed') == false
    }
  end
  {name: c['name'], results: values, checks: checks}
end
puts JSON.pretty_generate({ruby: RUBY_DESCRIPTION, psych: Psych::VERSION, cases: rows})
exit(rows.all? { |row| row[:results].values.all? { |result| result[:status] == 'PASS' } && row[:checks].length == 6 && row[:checks].values.all? } ? 0 : 1)

; HTTP methods
(method) @function.method

; Comments
(comment) @comment

; Metadata directives inside comments — `# @name foo`, `# @disabled`, etc.
(comment
  name: (identifier) @keyword)

; Request separators (### Optional name)
(request_separator) @comment
(request_separator
  value: (value) @label)

; URLs
(target_url) @string.special.url

; Headers
(header name: (header_entity) @property)
(header value: (value) @string)

; HTTP version, status
(http_version) @constant
(status_code) @number
(status_text) @string

; Variable interpolation `{{name}}` and declarations `@name = value`
(variable) @variable
(variable_declaration
  name: (identifier) @variable)
(variable_declaration
  "=" @operator)
(variable_declaration
  value: (value) @string)

; Bodies — captured here so themes can style them; injections.scm
; handles language-aware highlighting for json/xml/graphql.
(json_body) @string.special
(xml_body) @string.special
(graphql_body) @string.special
(raw_body) @string
(multipart_form_data) @string.special

; External body file references — `< ./file.json`
(external_body
  path: (path) @string.special.path)

; Pre-request and response handler script file paths — `< handler.js`, `> handler.js`
(pre_request_script
  (path) @string.special.path)
(res_handler_script
  (path) @string.special.path)

; Response redirects — `>> file.json`, `>>! file.json`
(res_redirect
  path: (path) @string.special.path)

; Outline: one entry per request section, named after `### <label>`
(section
  (request_separator
    value: (value) @name)
  request: (request) @context
) @item

; Sections without an explicit `### name` still get an entry, named by the URL
(section
  (request_separator) @context
  request: (request
    url: (target_url) @name)
) @item

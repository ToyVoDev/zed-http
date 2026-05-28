; Inject JSON highlighting into JSON request bodies
((json_body) @injection.content
 (#set! injection.language "json"))

; Inject XML highlighting into XML request bodies
((xml_body) @injection.content
 (#set! injection.language "xml"))

; Inject GraphQL highlighting into GraphQL request bodies
((graphql_body) @injection.content
 (#set! injection.language "graphql"))

; Inject JS highlighting into pre-request and response handler `{% ... %}` scripts
((script) @injection.content
 (#set! injection.language "javascript"))

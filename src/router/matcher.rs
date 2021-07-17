/// Result of a check made by a [Matcher]
/// [Matcher::OK] means the the request can be handled
/// [Matcher::UriOnly] means the request should have be handled, but the method is incorrect
/// [Matcher::KO] means the request cannot be handled
#[derive(Debug, PartialEq)]
pub enum MatcherResult {
    OK,
    UriOnly,
    KO,
}

/// Trait to implement to be able to tell whether a [crate::router::Handler] can handle a query or not
pub trait Matcher: Sync + Send {
    fn matches(&self, request: &hyper::Request<hyper::Body>) -> MatcherResult;
}

#[derive(Debug)]
pub enum MatcherBuilderError {
    IncorrectUri,
    IncorrectMethod,
}

enum UriMatcher {
    All,
    Exact(String),
    Regex(regex::Regex),
}

enum MethodMatcher {
    All,
    Exact(hyper::Method),
    Excluding(hyper::Method),
}

struct MatcherImpl {
    method_matcher: MethodMatcher,
    uri_matcher: UriMatcher,
}

impl Matcher for MatcherImpl {
    fn matches(&self, request: &hyper::Request<hyper::Body>) -> MatcherResult {
        let uri_match = match &self.uri_matcher {
            UriMatcher::All => true,
            UriMatcher::Exact(uri) => request.uri().path() == uri,
            UriMatcher::Regex(re) => re.is_match(request.uri().path()),
        };

        if uri_match {
            let method_match = match &self.method_matcher {
                MethodMatcher::All => true,
                MethodMatcher::Exact(method) => request.method() == method,
                MethodMatcher::Excluding(method) => request.method() != method,
            };

            return if method_match {
                MatcherResult::OK
            } else {
                MatcherResult::UriOnly
            };
        } else {
            MatcherResult::KO
        }
    }
}

pub struct MatcherBuilder {
    method_matcher: Option<MethodMatcher>,
    uri_matcher: Option<UriMatcher>,
}

pub fn builder() -> MatcherBuilder {
    MatcherBuilder::new()
}

impl MatcherBuilder {
    fn new() -> MatcherBuilder {
        MatcherBuilder {
            method_matcher: Some(MethodMatcher::All),
            uri_matcher: Some(UriMatcher::All),
        }
    }

    pub fn exact_path<T>(mut self, uri: T) -> MatcherBuilder
    where
        String: std::convert::TryFrom<T>,
    {
        let uri: std::result::Result<String, _> = std::convert::TryFrom::try_from(uri);
        self.uri_matcher = match uri {
            Ok(uri) => Some(UriMatcher::Exact(uri)),
            _ => None,
        };
        self
    }

    pub fn regex_path(mut self, regex: &str) -> MatcherBuilder {
        let regex = regex::Regex::new(regex);
        self.uri_matcher = match regex {
            Ok(regex) => Some(UriMatcher::Regex(regex)),
            _ => None,
        };
        self
    }

    pub fn with_method<T>(mut self, method: T) -> MatcherBuilder
    where
        hyper::Method: std::convert::TryFrom<T>,
    {
        let method: std::result::Result<hyper::Method, _> = std::convert::TryFrom::try_from(method);
        self.method_matcher = match method {
            Ok(method) => Some(MethodMatcher::Exact(method)),
            _ => None,
        };
        self
    }

    pub fn excluding_method<T>(mut self, method: T) -> MatcherBuilder
    where
        hyper::Method: std::convert::TryFrom<T>,
    {
        let method: std::result::Result<hyper::Method, _> = std::convert::TryFrom::try_from(method);
        self.method_matcher = match method {
            Ok(method) => Some(MethodMatcher::Excluding(method)),
            _ => None,
        };
        self
    }

    pub fn build(self) -> Result<Box<dyn Matcher>, MatcherBuilderError> {
        match self.method_matcher {
            None => Err(MatcherBuilderError::IncorrectMethod),
            Some(method_matcher) => match self.uri_matcher {
                None => Err(MatcherBuilderError::IncorrectUri),
                Some(uri_matcher) => Ok(Box::new(MatcherImpl {
                    method_matcher,
                    uri_matcher,
                })),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{builder, MatcherResult};

    fn get_request(uri: &str, method: &hyper::Method) -> hyper::Request<hyper::Body> {
        hyper::Request::builder()
            .uri(uri)
            .method(method)
            .body(hyper::Body::empty())
            .unwrap()
    }

    #[test]
    fn it_builds_exact_path_matchers() {
        let matcher = builder().exact_path("/test_uri").build().unwrap();
        let request = get_request("/test_uri", &hyper::Method::POST);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));

        let request = get_request("/test_uri", &hyper::Method::GET);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));

        let request = get_request("/bad_uri", &hyper::Method::GET);
        assert_eq!(MatcherResult::KO, matcher.matches(&request));
    }

    #[test]
    fn it_builds_regex_path_matchers() {
        let matcher = builder().regex_path("^/test_uri").build().unwrap();
        let request = get_request("/test_uri", &hyper::Method::POST);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));

        let request = get_request("/test_uri/many/more", &hyper::Method::POST);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));

        let request = get_request("/a/test_uri", &hyper::Method::GET);
        assert_eq!(MatcherResult::KO, matcher.matches(&request));
    }

    #[test]
    fn it_builds_method_matchers() {
        let matcher = builder().with_method("GET").build().unwrap();

        let request = get_request("/test_uri", &hyper::Method::GET);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));

        let request = get_request("/other_uri", &hyper::Method::GET);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));

        let request = get_request("/other_uri", &hyper::Method::POST);
        assert_eq!(MatcherResult::UriOnly, matcher.matches(&request));
    }

    #[test]
    fn it_builds_method_excluding_matchers() {
        let matcher = builder()
            .excluding_method(&hyper::Method::GET)
            .build()
            .unwrap();

        let request = get_request("/test_uri", &hyper::Method::GET);
        assert_eq!(MatcherResult::UriOnly, matcher.matches(&request));

        let request = get_request("/other_uri", &hyper::Method::POST);
        assert_eq!(MatcherResult::OK, matcher.matches(&request));
    }
}

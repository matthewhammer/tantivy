use core::schema::DocId;
use std::ptr;
use std::collections::BTreeMap;
use core::schema::Term;
use core::codec::SegmentSerializer;
use std::io;

pub struct PostingsWriter {
    postings: Vec<Vec<DocId>>,
    term_index: BTreeMap<Term, usize>,
}

impl PostingsWriter {

    pub fn new() -> PostingsWriter {
        PostingsWriter {
            postings: Vec::new(),
            term_index: BTreeMap::new(),
        }
    }

    pub fn suscribe(&mut self, doc: DocId, term: Term) {
        let doc_ids: &mut Vec<DocId> = self.get_term_postings(term);
        if doc_ids.len() == 0 || doc_ids[doc_ids.len() - 1] < doc {
			doc_ids.push(doc);
		}
    }

    fn get_term_postings(&mut self, term: Term) -> &mut Vec<DocId> {
        match self.term_index.get(&term) {
            Some(unord_id) => {
                return &mut self.postings[*unord_id];
            },
            None => {}
        }
        let unord_id = self.term_index.len();
        self.postings.push(Vec::new());
        self.term_index.insert(term, unord_id.clone());
        &mut self.postings[unord_id]
    }

    pub fn serialize(&self, serializer: &mut SegmentSerializer) -> io::Result<()> {
        for (term, postings_id) in self.term_index.iter() {
            let doc_ids = &self.postings[postings_id.clone()];
            let term_docfreq = doc_ids.len() as u32;
            try!(serializer.new_term(&term, term_docfreq));
            try!(serializer.write_docs(&doc_ids));
        }
        Ok(())
    }


}


//////////////////////////////////

pub trait Postings: Iterator<Item=DocId> {
    // after skipping position
    // the iterator in such a way that the
    // next call to next() will return a
    // value greater or equal to target.
    fn skip_next(&mut self, target: DocId) -> Option<DocId>;
}

pub struct IntersectionPostings<T: Postings> {
    postings: Vec<T>,
}

impl<T: Postings> IntersectionPostings<T> {
    pub fn from_postings(postings: Vec<T>) -> IntersectionPostings<T> {
        IntersectionPostings {
            postings: postings,
        }
    }
}

impl<T: Postings> Iterator for IntersectionPostings<T> {
    type Item = DocId;
    fn next(&mut self,) -> Option<DocId> {
        let mut candidate;
        match self.postings[0].next() {
            Some(val) => {
                candidate = val;
            },
            None => {
                return None;
            }
        }
        'outer: loop {
            for i in 1..self.postings.len() {
                let skip_result = self.postings[i].skip_next(candidate);
                match skip_result {
                    None => {
                        return None;
                    },
                    Some(x) if x == candidate => {
                    },
                    Some(greater) => {
                        unsafe {
                            let pa: *mut T = &mut self.postings[i];
                            let pb: *mut T = &mut self.postings[0];
                            ptr::swap(pa, pb);
                        }
                        candidate = greater;
                        continue 'outer;
                    },
                }
            }
            return Some(candidate);
        }

    }
}


#[cfg(test)]
mod tests {

    use super::*;
    use test::Bencher;
    use core::schema::DocId;


    #[derive(Debug)]
    pub struct VecPostings {
        doc_ids: Vec<DocId>,
    	cursor: usize,
    }

    impl VecPostings {
        pub fn new(vals: Vec<DocId>) -> VecPostings {
            VecPostings {
                doc_ids: vals,
    			cursor: 0,
            }
        }
    }

    impl Postings for VecPostings {
        // after skipping position
        // the iterator in such a way that the
        // next call to next() will return a
        // value greater or equal to target.
        fn skip_next(&mut self, target: DocId) -> Option<DocId> {
            loop {
                match Iterator::next(self) {
                    Some(val) if val >= target => {
                        return Some(val);
                    },
                    None => {
                        return None;
                    },
                    _ => {}
                }
            }
        }
    }

    impl Iterator for VecPostings {
    	type Item = DocId;
    	fn next(&mut self,) -> Option<DocId> {
    		if self.cursor >= self.doc_ids.len() {
    			None
    		}
    		else {
                self.cursor += 1;
    			Some(self.doc_ids[self.cursor - 1])
    		}
    	}
    }

    #[test]
    fn test_intersection() {
        {
            let left = VecPostings::new(vec!(1, 3, 9));
            let right = VecPostings::new(vec!(3, 4, 9, 18));
            let inter = IntersectionPostings::from_postings(vec!(left, right));
            let vals: Vec<DocId> = inter.collect();
            assert_eq!(vals, vec!(3, 9));
        }
        {
            let a = VecPostings::new(vec!(1, 3, 9));
            let b = VecPostings::new(vec!(3, 4, 9, 18));
            let c = VecPostings::new(vec!(1, 5, 9, 111));
            let inter = IntersectionPostings::from_postings(vec!(a, b, c));
            let vals: Vec<DocId> = inter.collect();
            assert_eq!(vals, vec!(9));
        }
    }

    #[bench]
    fn bench_single_intersection(b: &mut Bencher) {
        b.iter(|| {
            let docs = VecPostings::new((0..1_000_000).collect());
            let intersection = IntersectionPostings::from_postings(vec!(docs));
            intersection.count()
        });
    }
}

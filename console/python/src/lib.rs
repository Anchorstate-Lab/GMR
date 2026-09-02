use std::sync::Arc;

use gmr_api::{AnchorKey, Runtime, StatusId};
use gmr_console as core;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::Value;

fn failed(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}

fn ok<T>(outcome: Result<T, core::Fault>) -> PyResult<T> {
    outcome.map_err(failed)
}

fn taken(py: Python<'_>, value: Option<Bound<'_, PyAny>>) -> PyResult<Option<Value>> {
    let _ = py;
    value
        .map(|v| pythonize::depythonize(&v).map_err(|e| failed(e.to_string())))
        .transpose()
}

fn handed(py: Python<'_>, value: Value) -> PyResult<PyObject> {
    pythonize::pythonize(py, &value)
        .map(|b| b.unbind())
        .map_err(|e| failed(e.to_string()))
}

#[pyclass]
struct Gmr {
    rt: Arc<Runtime>,
    loop_: Arc<tokio::runtime::Runtime>,
}

impl Gmr {
    fn run<T>(
        &self,
        py: Python<'_>,
        work: impl std::future::Future<Output = PyResult<T>> + Send,
    ) -> PyResult<T>
    where
        T: Send,
    {
        let loop_ = Arc::clone(&self.loop_);
        py.allow_threads(|| loop_.block_on(work))
    }
}

#[pyfunction]
fn open(py: Python<'_>, options: Bound<'_, PyAny>) -> PyResult<Gmr> {
    let asked: core::Opening = ok(core::said(
        taken(py, Some(options))?.expect("an argument was passed"),
    ))?;
    let loop_ = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| failed(format!("cannot start the runtime loop: {e}")))?;
    let rt = py.allow_threads(|| loop_.block_on(core::opened(asked)));
    Ok(Gmr {
        rt: Arc::new(rt.map_err(failed)?),
        loop_: Arc::new(loop_),
    })
}

#[pymethods]
impl Gmr {
    #[pyo3(signature = (claims, how=None))]
    fn ground(
        &self,
        py: Python<'_>,
        claims: Vec<Bound<'_, PyAny>>,
        how: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let claims = claims
            .into_iter()
            .map(|c| {
                let value = taken(py, Some(c))?.expect("an element was passed");
                ok(core::asking(value))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let how = ok(core::asked(taken(py, how)?))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(rt.ground(&claims, &how).await))
        })?;
        handed(py, out)
    }

    #[pyo3(signature = (anchor, how=None))]
    fn sample(
        &self,
        py: Python<'_>,
        anchor: String,
        how: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let key = AnchorKey::new(anchor);
        let how = ok(core::asked(taken(py, how)?))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(rt.sample(&key, &how).await))
        })?;
        handed(py, out)
    }

    #[pyo3(signature = (anchor, how=None))]
    fn read(
        &self,
        py: Python<'_>,
        anchor: String,
        how: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let key = AnchorKey::new(anchor);
        let how = ok(core::asked(taken(py, how)?))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(rt.grounded_within(&key, &how).await))
        })?;
        handed(py, out)
    }

    #[pyo3(signature = (cursor, status=None))]
    fn since(&self, py: Python<'_>, cursor: u64, status: Option<String>) -> PyResult<PyObject> {
        let status = status.map(StatusId::new);
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(
                rt.changed_since(cursor, status.as_ref()).await,
            ))
        })?;
        handed(py, out)
    }

    #[pyo3(signature = (claim, anchors, source, how=None))]
    fn bind(
        &self,
        py: Python<'_>,
        claim: String,
        anchors: Vec<String>,
        source: String,
        how: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let how: core::Asserting = match taken(py, how)? {
            Some(stated) => ok(core::said(stated))?,
            None => core::Asserting::default(),
        };
        let (binding, bound_version, saw, source) = ok(core::bound(claim, anchors, &source, how))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(
                rt.bind(binding, bound_version, saw, source).await,
            ))
        })?;
        handed(py, out)
    }

    fn revoke(&self, py: Python<'_>, claim: String, source: String) -> PyResult<PyObject> {
        let claim = ok(core::named(claim))?;
        let source = ok(core::attested(&source))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(rt.revoke(&claim, source).await))
        })?;
        handed(py, out)
    }

    fn link(
        &self,
        py: Python<'_>,
        from_: String,
        to: String,
        kind: String,
        source: String,
    ) -> PyResult<()> {
        let from = ok(core::stored(from_))?;
        let to = ok(core::stored(to))?;
        let kind = gmr_api::LinkKind(kind);
        let source = ok(core::attested(&source))?;
        let rt = Arc::clone(&self.rt);
        self.run(py, async move {
            rt.link(&from, &to, kind, source)
                .await
                .map_err(|e| failed(e.to_string()))
        })
    }

    fn unlink(
        &self,
        py: Python<'_>,
        from_: String,
        to: String,
        kind: String,
        source: String,
    ) -> PyResult<u64> {
        let revocation = ok(core::revoking(from_, to, kind, &source, chrono::Utc::now()))?;
        let rt = Arc::clone(&self.rt);
        self.run(py, async move {
            rt.unlink(&revocation)
                .await
                .map_err(|e| failed(e.to_string()))
        })
    }

    fn anchors(&self, py: Python<'_>) -> PyResult<PyObject> {
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move { ok(core::answered(rt.sample_all().await)) })?;
        handed(py, out)
    }

    fn claims(&self, py: Python<'_>) -> PyResult<PyObject> {
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move { ok(core::answered(rt.claims().await)) })?;
        handed(py, out)
    }

    fn cobound(&self, py: Python<'_>, claim: String) -> PyResult<PyObject> {
        let claim = ok(core::named(claim))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(
            py,
            async move { ok(core::answered(rt.cobound(&claim).await)) },
        )?;
        handed(py, out)
    }

    fn links(&self, py: Python<'_>, record: String) -> PyResult<PyObject> {
        let record = ok(core::stored(record))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(
            py,
            async move { ok(core::answered(rt.links(&record).await)) },
        )?;
        handed(py, out)
    }

    fn condense(
        &self,
        py: Python<'_>,
        said: String,
        into: String,
        source: String,
    ) -> PyResult<PyObject> {
        let said = ok(core::uttered(said))?;
        let into = ok(core::stored(into))?;
        let source = ok(core::attested(&source))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(py, async move {
            ok(core::answered(rt.condense(&said, into, source).await))
        })?;
        handed(py, out)
    }

    #[pyo3(name = "open")]
    fn open_anchor(&self, py: Python<'_>, request: Bound<'_, PyAny>) -> PyResult<PyObject> {
        let request = ok(core::said(
            taken(py, Some(request))?.expect("an argument was passed"),
        ))?;
        let rt = Arc::clone(&self.rt);
        let out = self.run(
            py,
            async move { ok(core::answered(rt.open(request).await)) },
        )?;
        handed(py, out)
    }

    fn close(&self, py: Python<'_>, key: String, why: String) -> PyResult<()> {
        let key = AnchorKey::new(key);
        let rt = Arc::clone(&self.rt);
        self.run(py, async move {
            rt.close(&key, why.as_bytes())
                .await
                .map_err(|e| failed(e.to_string()))
        })
    }
}

#[pymodule]
fn gmr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("CONTRACT", gmr_api::contract::CONTRACT)?;
    m.add_class::<Gmr>()?;
    m.add_function(wrap_pyfunction!(open, m)?)?;
    Ok(())
}

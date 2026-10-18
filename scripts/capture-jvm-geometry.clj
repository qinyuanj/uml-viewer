;; JVM-only oracle. Does not load Quil, open a window, or touch mailboxes.
(require '[clojure.edn :as edn]
         '[clojure.pprint :as pp]
         '[clojure.string :as str]
         '[clojure.walk :as walk]
         '[uml-viewer.application.document :as document]
         '[uml-viewer.engine.curve :as curve])

(defn canonical [x]
  (walk/postwalk (fn [v] (if (map? v)
                          (into (sorted-map-by #(compare (pr-str %1) (pr-str %2))) v)
                          v)) x))

(defn edge-geometry [scene e]
  (let [idx (:class-by-id scene)
        path (-> (curve/basis-path (:points e))
                 (curve/constrain-ends (:rect (idx (:from e)))
                                       (:rect (idx (:to e)))))]
    (assoc (select-keys e [:from :to :kind :points :tip :behind :draw-bounds])
           :curve path)))

(defn capture-view [doc root focus]
  (let [scene (document/compile-view doc root focus)]
    {:focus focus :size (:size scene)
     :packages (mapv #(select-keys % [:id :rect]) (:packages scene))
     :classes (mapv #(select-keys % [:id :rect]) (:classes scene))
     :edges (mapv #(edge-geometry scene %) (:edges scene))}))

(doseq [language ["typescript" "rust" "python" "lua"]]
  (let [base (str (or (first *command-line-args*) "fixtures/jvm-goldens") "/" language)
        doc (edn/read-string (slurp (str base ".ir.edn")))
        ;; Every proper namespace prefix is a navigable hierarchy focus.
        focuses (->> (:classes doc)
                     (remove :foreign)
                     (mapcat (fn [c]
                               (let [parts (mapv keyword (str/split (name (:id c)) #"\."))]
                                 (map #(subvec parts 0 %) (range 1 (count parts))))))
                     distinct (sort-by pr-str) (cons []) vec)
        result {:version 1 :absolute-tolerance 1.0e-6
                :views (mapv #(capture-view doc (str language "-fixture") %) focuses)}]
    (spit (str base ".geometry.edn")
          (str ";; JVM viewer geometry; regenerate via scripts/regenerate-jvm-goldens.ps1\n"
               (binding [*print-namespace-maps* false pp/*print-right-margin* 100]
                 (with-out-str (pp/pprint (canonical result))))))
    (println "Captured" language (count focuses) "views")))

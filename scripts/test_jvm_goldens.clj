;; Run from the repository root: clj -M scripts/test_jvm_goldens.clj
(require '[clojure.edn :as edn]
         '[clojure.java.io :as io]
         '[clojure.test :refer [deftest is run-tests]])

(deftest four-language-baseline-is-usable
  (doseq [lang ["typescript" "rust" "python" "lua"]]
    (let [root (str "fixtures/jvm-goldens/" lang)
          ir (edn/read-string (slurp (str root ".ir.edn")))
          geometry (edn/read-string (slurp (str root ".geometry.edn")))]
      (is (seq (:classes ir)) lang)
      (is (seq (:edges ir)) lang)
      (is (= 1 (:version geometry)))
      (is (= 1.0e-6 (:absolute-tolerance geometry)))
      (is (seq (:views geometry)))
      (is (some #(seq (:edges %)) (:views geometry)))
      (is (every? #(every? (fn [edge] (and (seq (:points edge))
                                                   (seq (get-in edge [:curve :ops]))))
                            (:edges %)) (:views geometry)))
      (doseq [view (:views geometry)
              c (:classes view)]
        (is (every? number? (map (:rect c) [:x :y :w :h])))))))

(deftest scanner-scenarios-survive-ir-generation
  (let [read-ir #(edn/read-string (slurp (str "fixtures/jvm-goldens/" % ".ir.edn")))
        ids #(set (map :id (:classes %)))
        edges #(set (map (juxt :from :to :kind) (:edges %)))
        ts (read-ir "typescript") rs (read-ir "rust") py (read-ir "python")]
    (is (= #{:book :model :nodeFs :editor.view :tauriFs :markdown-it} (ids ts)))
    (is (contains? (edges ts) [:nodeFs :book :implements]))
    (is (contains? (edges rs) [:rust.main :rust :dependency]))
    (is (contains? (edges rs) [:rust.rec :rust.proto :implements]))
    (is (contains? (ids rs) :rust.proto.inner))
    (is (not (contains? (ids rs) :rust.ignored)))
    (is (contains? (edges py) [:dogs.kennel :model :inheritance]))
    (is (contains? (edges py) [:service :model :implements]))
    (is (not-any? #(re-find #"test|hidden|nope" (name %)) (ids py)))
    (is (some (fn [view]
                (some (fn [edge] (some #(= :cubic (:op %)) (get-in edge [:curve :ops])))
                      (:edges view)))
              (:views (edn/read-string (slurp "fixtures/jvm-goldens/typescript.geometry.edn")))))))

(let [{:keys [fail error]} (run-tests)]
  (System/exit (if (zero? (+ fail error)) 0 1)))

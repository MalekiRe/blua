local app = ...
function my_system(query_1)
    for transform in query_1:iter() do
        print(transform)
    end
end

function second_system(my_q)
    for t, stretch in my_q:iter() do
        print(t.translation.x)
        print(stretch)
        stretch.x = t.translation.x
    end
end

function third(q1, q2)
    for t in q1:iter() do
        print(t.translation.z)
    end

    for s in q2:iter() do
        print(s.x)
    end

end

app:register_system(my_system, { {bevy_transform.components.transform.Transform.mut} })
app:register_system(second_system, { {bevy_transform.components.transform.Transform.mut, simple_test.Stretch.mut} })
app:register_system(third, { {bevy_transform.components.transform.Transform.mut}, {simple_test.Stretch.mut} })
print("hello world!")
